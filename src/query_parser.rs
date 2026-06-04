use crate::error::{Result, SqlRockError};
use crate::model::{
    Aggregate, CompareOp, Condition, Expr, JoinClause, JoinKind, OrderBy, SelectItem, SelectQuery,
    SelectSource,
};

pub fn parse_select_query(sql: &str) -> Result<SelectQuery> {
    let mut parser = Parser::new(tokenize(sql)?);
    let query = parser.parse_query()?;
    if parser.peek().is_some() {
        return Err(parser.error("unexpected trailing SQL"));
    }
    Ok(query)
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    String(String),
    Number(String),
    Symbol(char),
    Operator(String),
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, index: 0 }
    }

    fn parse_query(&mut self) -> Result<SelectQuery> {
        self.expect_word("select")?;
        let distinct = self.consume_word("distinct");
        let items = self.parse_select_items()?;
        self.expect_word("from")?;
        let source = self.parse_source()?;
        let mut joins = Vec::new();

        while let Some(kind) = self.parse_join_kind() {
            let source = self.parse_source()?;
            let on = if kind == JoinKind::Cross {
                None
            } else {
                self.expect_word("on")?;
                Some(self.parse_condition()?)
            };
            joins.push(JoinClause { kind, source, on });
        }

        let where_clause = if self.consume_word("where") {
            Some(self.parse_condition()?)
        } else {
            None
        };

        let group_by = if self.consume_word("group") {
            self.expect_word("by")?;
            self.parse_expr_list()?
        } else {
            Vec::new()
        };

        let having = if self.consume_word("having") {
            Some(self.parse_condition()?)
        } else {
            None
        };

        let order_by = if self.consume_word("order") {
            self.expect_word("by")?;
            self.parse_order_by()?
        } else {
            Vec::new()
        };

        let limit = if self.consume_word("limit") {
            Some(self.expect_usize()?)
        } else {
            None
        };
        let offset = if self.consume_word("offset") {
            self.expect_usize()?
        } else {
            0
        };

        let union = if self.consume_word("union") {
            let all = self.consume_word("all");
            Some((all, Box::new(self.parse_query()?)))
        } else {
            None
        };

        Ok(SelectQuery {
            distinct,
            items,
            source,
            joins,
            where_clause,
            group_by,
            having,
            order_by,
            limit,
            offset,
            union,
        })
    }

    fn parse_select_items(&mut self) -> Result<Vec<SelectItem>> {
        let mut items = Vec::new();
        loop {
            let expr = self.parse_expr()?;
            let alias = if self.consume_word("as") {
                Some(self.expect_identifier()?)
            } else if self.peek_word().is_some_and(|word| !is_clause_word(word)) {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            items.push(SelectItem { expr, alias });

            if !self.consume_symbol(',') {
                break;
            }
        }
        Ok(items)
    }

    fn parse_source(&mut self) -> Result<SelectSource> {
        if self.consume_symbol('(') {
            let query = self.parse_query()?;
            self.expect_symbol(')')?;
            let alias = self.expect_identifier()?;
            return Ok(SelectSource::Subquery {
                query: Box::new(query),
                alias,
            });
        }

        let name = self.expect_identifier()?;
        let alias = if self.consume_word("as") {
            self.expect_identifier()?
        } else if self.peek_word().is_some_and(|word| !is_clause_word(word)) {
            self.expect_identifier()?
        } else {
            name.clone()
        };
        Ok(SelectSource::Table { name, alias })
    }

    fn parse_join_kind(&mut self) -> Option<JoinKind> {
        if self.consume_word("inner") {
            self.consume_word("join");
            Some(JoinKind::Inner)
        } else if self.consume_word("left") {
            self.consume_word("join");
            Some(JoinKind::Left)
        } else if self.consume_word("right") {
            self.consume_word("join");
            Some(JoinKind::Right)
        } else if self.consume_word("cross") {
            self.consume_word("join");
            Some(JoinKind::Cross)
        } else if self.consume_word("join") {
            Some(JoinKind::Inner)
        } else {
            None
        }
    }

    fn parse_condition(&mut self) -> Result<Condition> {
        let mut condition = self.parse_and_condition()?;
        while self.consume_word("or") {
            condition = Condition::Or(Box::new(condition), Box::new(self.parse_and_condition()?));
        }
        Ok(condition)
    }

    fn parse_and_condition(&mut self) -> Result<Condition> {
        let mut condition = self.parse_single_condition()?;
        while self.consume_word("and") {
            condition = Condition::And(
                Box::new(condition),
                Box::new(self.parse_single_condition()?),
            );
        }
        Ok(condition)
    }

    fn parse_single_condition(&mut self) -> Result<Condition> {
        if self.consume_word("exists") {
            self.expect_symbol('(')?;
            let query = self.parse_query()?;
            self.expect_symbol(')')?;
            return Ok(Condition::Exists(Box::new(query)));
        }

        if self.consume_symbol('(') {
            let condition = self.parse_condition()?;
            self.expect_symbol(')')?;
            return Ok(condition);
        }

        let left = self.parse_expr()?;
        if self.consume_word("between") {
            let start = self.parse_expr()?;
            self.expect_word("and")?;
            let end = self.parse_expr()?;
            return Ok(Condition::Between(left, start, end));
        }
        if self.consume_word("in") {
            self.expect_symbol('(')?;
            if self
                .peek_word()
                .is_some_and(|word| word.eq_ignore_ascii_case("select"))
            {
                let query = self.parse_query()?;
                self.expect_symbol(')')?;
                return Ok(Condition::InQuery(left, Box::new(query)));
            }
            let values = self.parse_expr_list()?;
            self.expect_symbol(')')?;
            return Ok(Condition::InValues(left, values));
        }
        if self.consume_word("like") {
            return Ok(Condition::Like(left, self.expect_string()?));
        }
        if self.consume_word("is") {
            let not = self.consume_word("not");
            self.expect_word("null")?;
            return Ok(Condition::IsNull(left, not));
        }

        let operator = self.expect_operator()?;
        let right = self.parse_expr()?;
        Ok(Condition::Compare(left, operator, right))
    }

    fn parse_expr_list(&mut self) -> Result<Vec<Expr>> {
        let mut expressions = Vec::new();
        loop {
            expressions.push(self.parse_expr()?);
            if !self.consume_symbol(',') {
                break;
            }
        }
        Ok(expressions)
    }

    fn parse_order_by(&mut self) -> Result<Vec<OrderBy>> {
        let mut orders = Vec::new();
        loop {
            let expr = self.parse_expr()?;
            let descending = if self.consume_word("desc") {
                true
            } else {
                self.consume_word("asc");
                false
            };
            orders.push(OrderBy { expr, descending });
            if !self.consume_symbol(',') {
                break;
            }
        }
        Ok(orders)
    }

    fn parse_expr(&mut self) -> Result<Expr> {
        if self.consume_symbol('*') {
            return Ok(Expr::All);
        }
        if self.consume_word("case") {
            return self.parse_case();
        }
        if let Some(word) = self.peek_word() {
            if let Some(aggregate) = parse_aggregate(word) {
                self.index += 1;
                self.expect_symbol('(')?;
                let expr = self.parse_expr()?;
                self.expect_symbol(')')?;
                return Ok(Expr::Aggregate(aggregate, Box::new(expr)));
            }
        }

        match self.next() {
            Some(Token::String(value)) | Some(Token::Number(value)) => Ok(Expr::Literal(value)),
            Some(Token::Word(word)) if word.eq_ignore_ascii_case("null") => {
                Ok(Expr::Literal(String::new()))
            }
            Some(Token::Word(word)) if word.eq_ignore_ascii_case("now") => {
                self.expect_symbol('(')?;
                self.expect_symbol(')')?;
                Ok(Expr::Now)
            }
            Some(Token::Word(word)) if word.eq_ignore_ascii_case("today") => {
                self.expect_symbol('(')?;
                self.expect_symbol(')')?;
                Ok(Expr::Today)
            }
            Some(Token::Word(mut word)) => {
                if self.consume_symbol('.') {
                    word.push('.');
                    word.push_str(&self.expect_identifier()?);
                }
                Ok(Expr::Column(word))
            }
            _ => Err(self.error("expected expression")),
        }
    }

    fn parse_case(&mut self) -> Result<Expr> {
        let mut branches = Vec::new();
        while self.consume_word("when") {
            let condition = self.parse_condition()?;
            self.expect_word("then")?;
            branches.push((condition, Box::new(self.parse_expr()?)));
        }
        self.expect_word("else")?;
        let fallback = Box::new(self.parse_expr()?);
        self.expect_word("end")?;
        Ok(Expr::Case { branches, fallback })
    }

    fn expect_operator(&mut self) -> Result<CompareOp> {
        match self.next() {
            Some(Token::Operator(operator)) => match operator.as_str() {
                "=" => Ok(CompareOp::Eq),
                "!=" | "<>" => Ok(CompareOp::NotEq),
                ">" => Ok(CompareOp::Greater),
                "<" => Ok(CompareOp::Less),
                ">=" => Ok(CompareOp::GreaterEq),
                "<=" => Ok(CompareOp::LessEq),
                _ => Err(self.error("unsupported comparison operator")),
            },
            _ => Err(self.error("expected comparison operator")),
        }
    }

    fn expect_identifier(&mut self) -> Result<String> {
        match self.next() {
            Some(Token::Word(word)) => Ok(word),
            _ => Err(self.error("expected identifier")),
        }
    }

    fn expect_string(&mut self) -> Result<String> {
        match self.next() {
            Some(Token::String(value)) => Ok(value),
            _ => Err(self.error("expected string value")),
        }
    }

    fn expect_usize(&mut self) -> Result<usize> {
        match self.next() {
            Some(Token::Number(value)) => value
                .parse()
                .map_err(|_| self.error("expected unsigned integer")),
            _ => Err(self.error("expected unsigned integer")),
        }
    }

    fn expect_word(&mut self, expected: &str) -> Result<()> {
        if self.consume_word(expected) {
            Ok(())
        } else {
            Err(self.error(&format!("expected keyword `{expected}`")))
        }
    }

    fn expect_symbol(&mut self, expected: char) -> Result<()> {
        if self.consume_symbol(expected) {
            Ok(())
        } else {
            Err(self.error(&format!("expected `{expected}`")))
        }
    }

    fn consume_word(&mut self, expected: &str) -> bool {
        if self
            .peek_word()
            .is_some_and(|word| word.eq_ignore_ascii_case(expected))
        {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_symbol(&mut self, expected: char) -> bool {
        if self.peek() == Some(&Token::Symbol(expected)) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn peek_word(&self) -> Option<&str> {
        match self.peek() {
            Some(Token::Word(word)) => Some(word),
            _ => None,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned();
        self.index += usize::from(token.is_some());
        token
    }

    fn error(&self, message: &str) -> SqlRockError {
        SqlRockError::new(message)
    }
}

fn tokenize(sql: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = sql.trim().trim_end_matches(';').chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '\'' {
            let mut value = String::new();
            loop {
                let Some(next) = chars.next() else {
                    return Err(SqlRockError::new("unterminated string value"));
                };
                if next == '\'' {
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                        value.push('\'');
                    } else {
                        break;
                    }
                } else {
                    value.push(next);
                }
            }
            tokens.push(Token::String(value));
        } else if ch == '`' {
            let mut word = String::new();
            loop {
                let Some(next) = chars.next() else {
                    return Err(SqlRockError::new("unterminated quoted identifier"));
                };
                if next == '`' {
                    if chars.peek() == Some(&'`') {
                        chars.next();
                        word.push('`');
                    } else {
                        break;
                    }
                } else {
                    word.push(next);
                }
            }
            tokens.push(Token::Word(word));
        } else if ch.is_ascii_digit()
            || (ch == '-' && chars.peek().is_some_and(|c| c.is_ascii_digit()))
        {
            let mut value = String::from(ch);
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_digit() || *c == '.')
            {
                value.push(chars.next().expect("peeked character exists"));
            }
            tokens.push(Token::Number(value));
        } else if ch.is_ascii_alphabetic() || ch == '_' {
            let mut word = String::from(ch);
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                word.push(chars.next().expect("peeked character exists"));
            }
            tokens.push(Token::Word(word));
        } else if matches!(ch, '=' | '!' | '<' | '>') {
            let mut operator = String::from(ch);
            if chars
                .peek()
                .is_some_and(|c| *c == '=' || (ch == '<' && *c == '>'))
            {
                operator.push(chars.next().expect("peeked character exists"));
            }
            tokens.push(Token::Operator(operator));
        } else if matches!(ch, '(' | ')' | ',' | '*' | '.') {
            tokens.push(Token::Symbol(ch));
        } else {
            return Err(SqlRockError::new(format!("unexpected character `{ch}`")));
        }
    }
    Ok(tokens)
}

fn parse_aggregate(word: &str) -> Option<Aggregate> {
    if word.eq_ignore_ascii_case("count") {
        Some(Aggregate::Count)
    } else if word.eq_ignore_ascii_case("sum") {
        Some(Aggregate::Sum)
    } else if word.eq_ignore_ascii_case("avg") {
        Some(Aggregate::Avg)
    } else if word.eq_ignore_ascii_case("max") {
        Some(Aggregate::Max)
    } else if word.eq_ignore_ascii_case("min") {
        Some(Aggregate::Min)
    } else {
        None
    }
}

fn is_clause_word(word: &str) -> bool {
    [
        "from", "where", "group", "having", "order", "limit", "offset", "union", "inner", "left",
        "right", "cross", "join", "on", "asc", "desc", "when", "then", "else", "end",
    ]
    .iter()
    .any(|keyword| word.eq_ignore_ascii_case(keyword))
}
