use crate::error::{Result, SqlRockError};
use crate::model::{Column, SetClause, Statement, WhereClause};

pub fn parse_statement(sql: &str) -> Result<Statement> {
    let sql = sql.trim().trim_end_matches(';').trim();

    if starts_with_keyword(sql, "create table") {
        parse_create_table(sql)
    } else if starts_with_keyword(sql, "insert into") {
        parse_insert_into(sql)
    } else if starts_with_keyword(sql, "select") {
        parse_select(sql)
    } else if starts_with_keyword(sql, "desc") {
        parse_desc(sql)
    } else if starts_with_keyword(sql, "drop table") {
        parse_drop_table(sql)
    } else if starts_with_keyword(sql, "delete from") {
        parse_delete_from(sql)
    } else if starts_with_keyword(sql, "update") {
        parse_update(sql)
    } else {
        Err(SqlRockError::new(
            "unsupported SQL statement. supported: CREATE TABLE, INSERT INTO, SELECT, DESC, DROP TABLE, DELETE FROM, UPDATE",
        ))
    }
}

pub fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            current.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                current.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if ch == ';' && !in_string {
            let statement = current.trim();
            if !statement.is_empty() {
                statements.push(statement.to_string());
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }

    let statement = current.trim();
    if !statement.is_empty() {
        statements.push(statement.to_string());
    }

    statements
}

fn parse_create_table(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "create table")?.trim();
    let (table_name, definition) = split_name_and_parenthesized(rest)?;
    validate_identifier(table_name)?;

    let mut columns = Vec::new();
    for item in split_comma_separated(definition) {
        let mut parts = item.split_whitespace();
        let Some(name) = parts.next() else {
            return Err(SqlRockError::new("empty column definition"));
        };
        validate_identifier(name)?;

        let data_type = parts.collect::<Vec<_>>().join(" ");
        if data_type.is_empty() {
            return Err(SqlRockError::new(format!(
                "column `{name}` requires a data type"
            )));
        }

        columns.push(Column {
            name: name.to_string(),
            data_type,
        });
    }

    if columns.is_empty() {
        return Err(SqlRockError::new(
            "CREATE TABLE requires at least one column",
        ));
    }

    Ok(Statement::CreateTable {
        name: table_name.to_string(),
        columns,
    })
}

fn parse_insert_into(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "insert into")?.trim();
    let (table_name, rest) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let rest = rest.trim_start();
    if !rest.starts_with('(') {
        return Err(SqlRockError::new(
            "INSERT INTO requires an explicit column list: INSERT INTO table (col) VALUES (...)",
        ));
    }

    let (columns_sql, after_columns) = take_parenthesized(rest)?;
    let after_values = strip_keyword(after_columns.trim_start(), "values")?.trim_start();
    let (values_sql, trailing) = take_parenthesized(after_values)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }

    let columns = split_comma_separated(columns_sql)
        .into_iter()
        .map(|column| {
            validate_identifier(&column)?;
            Ok(column)
        })
        .collect::<Result<Vec<_>>>()?;

    let values = split_comma_separated(values_sql)
        .into_iter()
        .map(|value| parse_value(&value))
        .collect::<Result<Vec<_>>>()?;

    Ok(Statement::InsertInto {
        table: table_name.to_string(),
        columns,
        values,
    })
}

fn parse_select(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "select")?.trim_start();
    if rest.starts_with('*') {
        return parse_select_all(rest);
    }

    if starts_with_keyword(rest, "count") {
        return parse_select_count(rest);
    }

    Err(SqlRockError::new(
        "unsupported SELECT expression. supported: SELECT * FROM, SELECT count(column) FROM",
    ))
}

fn parse_select_all(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "*")?.trim_start();
    let rest = strip_keyword(rest, "from")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let trailing = trailing.trim_start();
    let where_clause = if trailing.is_empty() {
        None
    } else {
        Some(parse_where_clause(trailing)?)
    };

    Ok(Statement::SelectAll {
        table: table_name.to_string(),
        where_clause,
    })
}

fn parse_select_count(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "count")?.trim_start();
    let (column_name, rest) = take_parenthesized(rest)?;
    let column_name = column_name.trim();
    validate_identifier(column_name)?;

    let rest = strip_keyword(rest.trim_start(), "from")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let trailing = trailing.trim_start();
    let where_clause = if trailing.is_empty() {
        None
    } else {
        Some(parse_where_clause(trailing)?)
    };

    Ok(Statement::SelectCount {
        table: table_name.to_string(),
        column: column_name.to_string(),
        where_clause,
    })
}

fn parse_desc(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "desc")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }

    Ok(Statement::DescribeTable {
        table: table_name.to_string(),
    })
}

fn parse_drop_table(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "drop table")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }

    Ok(Statement::DropTable {
        table: table_name.to_string(),
    })
}

fn parse_delete_from(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "delete from")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let where_clause = parse_where_clause(trailing.trim_start())?;
    Ok(Statement::DeleteFrom {
        table: table_name.to_string(),
        where_clause,
    })
}

fn parse_update(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "update")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let trailing = trailing.trim_start();
    let after_set = strip_keyword(trailing, "set")?.trim_start();
    let Some((set_sql, where_sql)) = split_once_keyword(after_set, "where") else {
        return Err(SqlRockError::new(
            "UPDATE requires WHERE: UPDATE table SET column = value WHERE column = value",
        ));
    };

    let set_clause = parse_set_clause(set_sql.trim())?;
    let where_clause = parse_where_clause(where_sql.trim_start())?;

    Ok(Statement::Update {
        table: table_name.to_string(),
        set_clause,
        where_clause,
    })
}

fn parse_where_clause(sql: &str) -> Result<WhereClause> {
    let rest = strip_keyword(sql, "where")?.trim_start();
    let (column_name, rest) = split_leading_identifier(rest)?;
    validate_identifier(column_name)?;

    let rest = rest.trim_start();
    let rest = strip_keyword(rest, "=")?.trim_start();
    if rest.is_empty() {
        return Err(SqlRockError::new(
            "WHERE clause requires a value: WHERE column = value",
        ));
    }

    let value = parse_value(rest)?;
    Ok(WhereClause {
        column: column_name.to_string(),
        value,
    })
}

fn parse_set_clause(sql: &str) -> Result<SetClause> {
    let Some((column_name, value_sql)) = sql.split_once('=') else {
        return Err(SqlRockError::new(
            "SET clause requires a value: SET column = value",
        ));
    };
    let column_name = column_name.trim();
    validate_identifier(column_name)?;
    let value = parse_value(value_sql.trim())?;

    Ok(SetClause {
        column: column_name.to_string(),
        value,
    })
}

fn parse_value(value: &str) -> Result<String> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("null") {
        return Ok(String::new());
    }

    if value.starts_with('\'') {
        if !value.ends_with('\'') || value.len() < 2 {
            return Err(SqlRockError::new(format!(
                "unterminated string value: {value}"
            )));
        }

        return Ok(value[1..value.len() - 1].replace("''", "'"));
    }

    Ok(value.to_string())
}

fn split_name_and_parenthesized(input: &str) -> Result<(&str, &str)> {
    let (name, rest) = split_leading_identifier(input)?;
    let rest = rest.trim_start();
    let (inside, trailing) = take_parenthesized(rest)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }
    Ok((name, inside))
}

fn split_leading_identifier(input: &str) -> Result<(&str, &str)> {
    let input = input.trim_start();
    let end = input
        .char_indices()
        .find(|(_, ch)| !is_identifier_char(*ch))
        .map(|(index, _)| index)
        .unwrap_or(input.len());

    if end == 0 {
        return Err(SqlRockError::new("expected identifier"));
    }

    Ok((&input[..end], &input[end..]))
}

fn take_parenthesized(input: &str) -> Result<(&str, &str)> {
    let input = input.trim_start();
    if !input.starts_with('(') {
        return Err(SqlRockError::new("expected `(`"));
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut chars = input.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch == '\'' {
            if in_string && chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
            } else {
                in_string = !in_string;
            }
        } else if !in_string && ch == '(' {
            depth += 1;
        } else if !in_string && ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Ok((&input[1..index], &input[index + 1..]));
            }
        }
    }

    Err(SqlRockError::new("missing closing `)`"))
}

fn split_comma_separated(input: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    let mut in_string = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            current.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                current.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if !in_string && ch == '(' {
            depth += 1;
            current.push(ch);
        } else if !in_string && ch == ')' {
            depth -= 1;
            current.push(ch);
        } else if !in_string && depth == 0 && ch == ',' {
            items.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }

    let item = current.trim();
    if !item.is_empty() {
        items.push(item.to_string());
    }

    items
}

fn split_once_keyword<'a>(input: &'a str, keyword: &str) -> Option<(&'a str, &'a str)> {
    let mut in_string = false;
    let mut chars = input.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch == '\'' {
            if in_string && chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
            } else {
                in_string = !in_string;
            }
            continue;
        }

        if in_string {
            continue;
        }

        let rest = &input[index..];
        if rest
            .get(..keyword.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
        {
            return Some((&input[..index], rest));
        }
    }

    None
}

fn strip_keyword<'a>(input: &'a str, keyword: &str) -> Result<&'a str> {
    if starts_with_keyword(input, keyword) {
        Ok(&input[keyword.len()..])
    } else {
        Err(SqlRockError::new(format!("expected keyword `{keyword}`")))
    }
}

fn starts_with_keyword(input: &str, keyword: &str) -> bool {
    input
        .get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return Err(SqlRockError::new("identifier cannot be empty"));
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(SqlRockError::new(format!(
            "invalid identifier `{identifier}`"
        )));
    }

    if chars.any(|ch| !is_identifier_char(ch)) {
        return Err(SqlRockError::new(format!(
            "invalid identifier `{identifier}`"
        )));
    }

    Ok(())
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}
