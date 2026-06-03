use crate::data_type::{
    has_primary_key, has_unique_key, validate_auto_increment_columns, validate_data_type,
    validate_key_columns,
};
use crate::error::{Result, SqlRockError};
use crate::model::{AlterTableAction, Column, SQL_NULL, SetClause, Statement, WhereClause};
use crate::query_parser::parse_select_query;

pub fn parse_statement(sql: &str) -> Result<Statement> {
    let sql = sql.trim().trim_end_matches(';').trim();

    if starts_with_keyword(sql, "create table") {
        parse_create_table(sql)
    } else if starts_with_keyword(sql, "insert into") {
        parse_insert_into(sql)
    } else if starts_with_keyword(sql, "select") {
        parse_select(sql)
    } else if starts_with_keyword(sql, "describe") {
        parse_describe(sql)
    } else if starts_with_keyword(sql, "desc") {
        parse_desc(sql)
    } else if starts_with_keyword(sql, "show tables") {
        parse_show_tables(sql)
    } else if starts_with_keyword(sql, "show create table") {
        parse_show_create_table(sql)
    } else if starts_with_keyword(sql, "alter table") {
        parse_alter_table(sql)
    } else if starts_with_keyword(sql, "drop table") {
        parse_drop_table(sql)
    } else if starts_with_keyword(sql, "delete from") {
        parse_delete_from(sql)
    } else if starts_with_keyword(sql, "update") {
        parse_update(sql)
    } else if starts_with_keyword(sql, "truncate table") {
        parse_truncate_table(sql)
    } else {
        Err(SqlRockError::new("unsupported SQL statement"))
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
    let mut key_constraints = Vec::new();
    for item in split_comma_separated(definition) {
        if let Some(key_constraint) = parse_table_key_constraint(&item)? {
            key_constraints.push(key_constraint);
            continue;
        }

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
        validate_data_type(&data_type)?;

        columns.push(Column {
            name: name.to_string(),
            data_type,
        });
    }
    apply_table_key_constraints(&mut columns, key_constraints)?;

    if columns.is_empty() {
        return Err(SqlRockError::new(
            "CREATE TABLE requires at least one column",
        ));
    }
    validate_auto_increment_columns(&columns)?;
    validate_key_columns(&columns)?;

    Ok(Statement::CreateTable {
        name: table_name.to_string(),
        columns,
    })
}

enum TableKeyConstraint {
    PrimaryKey(String),
    UniqueKey(String),
}

fn parse_table_key_constraint(item: &str) -> Result<Option<TableKeyConstraint>> {
    let item = item.trim();
    if starts_with_keyword(item, "primary key") {
        let rest = strip_keyword(item, "primary key")?.trim_start();
        let column = parse_single_key_column(rest, "PRIMARY KEY")?;
        return Ok(Some(TableKeyConstraint::PrimaryKey(column)));
    }

    if starts_with_keyword(item, "unique key") {
        let rest = strip_keyword(item, "unique key")?.trim_start();
        let column = parse_optional_named_key_column(rest, "UNIQUE KEY")?;
        return Ok(Some(TableKeyConstraint::UniqueKey(column)));
    }

    if starts_with_keyword(item, "unique") {
        let rest = strip_keyword(item, "unique")?.trim_start();
        let rest = if starts_with_keyword(rest, "key") {
            strip_keyword(rest, "key")?.trim_start()
        } else {
            rest
        };
        let column = parse_optional_named_key_column(rest, "UNIQUE")?;
        return Ok(Some(TableKeyConstraint::UniqueKey(column)));
    }

    Ok(None)
}

fn parse_optional_named_key_column<'a>(rest: &'a str, constraint_name: &str) -> Result<String> {
    if rest.trim_start().starts_with('(') {
        return parse_single_key_column(rest, constraint_name);
    }

    let (_key_name, rest) = split_leading_identifier(rest)?;
    parse_single_key_column(rest.trim_start(), constraint_name)
}

fn parse_single_key_column(rest: &str, constraint_name: &str) -> Result<String> {
    let (columns_sql, trailing) = take_parenthesized(rest)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL in {constraint_name}"
        )));
    }

    let columns = split_comma_separated(columns_sql);
    if columns.len() != 1 {
        return Err(SqlRockError::new(format!(
            "{constraint_name} supports exactly one column"
        )));
    }

    let column = columns[0].trim();
    validate_identifier(column)?;
    Ok(column.to_string())
}

fn apply_table_key_constraints(
    columns: &mut [Column],
    key_constraints: Vec<TableKeyConstraint>,
) -> Result<()> {
    for key_constraint in key_constraints {
        let (column_name, attribute) = match key_constraint {
            TableKeyConstraint::PrimaryKey(column_name) => (column_name, "PRIMARY KEY"),
            TableKeyConstraint::UniqueKey(column_name) => (column_name, "UNIQUE KEY"),
        };
        let Some(column) = columns
            .iter_mut()
            .find(|column| column.name.eq_ignore_ascii_case(&column_name))
        else {
            return Err(SqlRockError::new(format!(
                "unknown column `{column_name}` in key definition"
            )));
        };

        let already_has_attribute = match attribute {
            "PRIMARY KEY" => has_primary_key(&column.data_type),
            "UNIQUE KEY" => has_unique_key(&column.data_type),
            _ => false,
        };
        if !already_has_attribute {
            column.data_type = format!("{} {attribute}", column.data_type);
        }
    }

    Ok(())
}

fn parse_insert_into(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "insert into")?.trim();
    let (table_name, rest) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    let rest = rest.trim_start();
    if starts_with_keyword(rest, "select") {
        return Ok(Statement::InsertSelect {
            table: table_name.to_string(),
            query: Box::new(parse_select_query(rest)?),
        });
    }
    if !rest.starts_with('(') {
        return Err(SqlRockError::new(
            "INSERT INTO requires an explicit column list: INSERT INTO table (col) VALUES (...)",
        ));
    }

    let (columns_sql, after_columns) = take_parenthesized(rest)?;
    let after_values = strip_keyword(after_columns.trim_start(), "values")?.trim_start();
    let columns = split_comma_separated(columns_sql)
        .into_iter()
        .map(|column| {
            validate_identifier(&column)?;
            Ok(column)
        })
        .collect::<Result<Vec<_>>>()?;

    let mut rows = Vec::new();
    let mut trailing = after_values;
    loop {
        let (values_sql, rest) = take_parenthesized(trailing)?;
        rows.push(
            split_comma_separated(values_sql)
                .into_iter()
                .map(|value| parse_value(&value))
                .collect::<Result<Vec<_>>>()?,
        );
        trailing = rest.trim_start();
        if !trailing.starts_with(',') {
            break;
        }
        trailing = trailing[1..].trim_start();
    }
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }
    if rows.len() == 1 {
        Ok(Statement::InsertInto {
            table: table_name.to_string(),
            columns,
            values: rows.remove(0),
        })
    } else {
        Ok(Statement::InsertRows {
            table: table_name.to_string(),
            columns,
            rows,
        })
    }
}

fn parse_select(sql: &str) -> Result<Statement> {
    if requires_query_engine(sql) {
        return Ok(Statement::SelectQuery(Box::new(parse_select_query(sql)?)));
    }

    let rest = strip_keyword(sql, "select")?.trim_start();
    if rest.starts_with('*') {
        return parse_select_all(rest)
            .or_else(|_| Ok(Statement::SelectQuery(Box::new(parse_select_query(sql)?))));
    }

    if starts_with_keyword(rest, "count") {
        return parse_select_count(rest)
            .or_else(|_| Ok(Statement::SelectQuery(Box::new(parse_select_query(sql)?))));
    }

    Ok(Statement::SelectQuery(Box::new(parse_select_query(sql)?)))
}

fn requires_query_engine(sql: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    [
        " distinct ",
        " order ",
        " limit ",
        " group ",
        " having ",
        " join ",
        " union ",
        " case ",
        " between ",
        " in ",
        " like ",
        " is ",
        " exists ",
        " and ",
        " or ",
        ">",
        "<",
        "!=",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
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

fn parse_describe(sql: &str) -> Result<Statement> {
    parse_table_only(sql, "describe", |table| Statement::DescribeTable { table })
}

fn parse_show_tables(sql: &str) -> Result<Statement> {
    let trailing = strip_keyword(sql, "show tables")?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new("unexpected trailing SQL"));
    }
    Ok(Statement::ShowTables)
}

fn parse_show_create_table(sql: &str) -> Result<Statement> {
    parse_table_only(sql, "show create table", |table| {
        Statement::ShowCreateTable { table }
    })
}

fn parse_alter_table(sql: &str) -> Result<Statement> {
    let rest = strip_keyword(sql, "alter table")?.trim_start();
    let (table, rest) = split_leading_identifier(rest)?;
    validate_identifier(table)?;
    let rest = rest.trim_start();
    let action = if starts_with_keyword(rest, "add column") {
        AlterTableAction::Add(parse_column_definition(
            strip_keyword(rest, "add column")?.trim_start(),
        )?)
    } else if starts_with_keyword(rest, "modify column") {
        AlterTableAction::Modify(parse_column_definition(
            strip_keyword(rest, "modify column")?.trim_start(),
        )?)
    } else if starts_with_keyword(rest, "change column") {
        let rest = strip_keyword(rest, "change column")?.trim_start();
        let (old_name, rest) = split_leading_identifier(rest)?;
        validate_identifier(old_name)?;
        AlterTableAction::Change {
            old_name: old_name.to_string(),
            column: parse_column_definition(rest.trim_start())?,
        }
    } else {
        return Err(SqlRockError::new("unsupported ALTER TABLE action"));
    };
    Ok(Statement::AlterTable {
        table: table.to_string(),
        action,
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

    if trailing.trim().is_empty() {
        Ok(Statement::DeleteAll {
            table: table_name.to_string(),
        })
    } else {
        let where_clause = parse_where_clause(trailing.trim_start())?;
        Ok(Statement::DeleteFrom {
            table: table_name.to_string(),
            where_clause,
        })
    }
}

fn parse_truncate_table(sql: &str) -> Result<Statement> {
    parse_table_only(sql, "truncate table", |table| Statement::TruncateTable {
        table,
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

    let set_clauses = split_comma_separated(set_sql)
        .into_iter()
        .map(|set_sql| parse_set_clause(&set_sql))
        .collect::<Result<Vec<_>>>()?;
    let where_clause = parse_where_clause(where_sql.trim_start())?;

    if set_clauses.len() == 1 {
        Ok(Statement::Update {
            table: table_name.to_string(),
            set_clause: set_clauses.into_iter().next().expect("checked length"),
            where_clause,
        })
    } else {
        Ok(Statement::UpdateMany {
            table: table_name.to_string(),
            set_clauses,
            where_clause,
        })
    }
}

fn parse_table_only(
    sql: &str,
    keyword: &str,
    build: impl FnOnce(String) -> Statement,
) -> Result<Statement> {
    let rest = strip_keyword(sql, keyword)?.trim_start();
    let (table, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new("unexpected trailing SQL"));
    }
    Ok(build(table.to_string()))
}

fn parse_column_definition(sql: &str) -> Result<Column> {
    let mut parts = sql.split_whitespace();
    let name = parts
        .next()
        .ok_or_else(|| SqlRockError::new("expected column name"))?;
    validate_identifier(name)?;
    let data_type = parts.collect::<Vec<_>>().join(" ");
    if data_type.is_empty() {
        return Err(SqlRockError::new("expected column type"));
    }
    validate_data_type(&data_type)?;
    Ok(Column {
        name: name.to_string(),
        data_type,
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
        return Ok(SQL_NULL.to_string());
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
