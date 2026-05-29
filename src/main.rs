use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

type Result<T> = std::result::Result<T, SqlRockError>;

#[derive(Debug)]
struct SqlRockError {
    message: String,
}

impl SqlRockError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SqlRockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for SqlRockError {}

impl From<io::Error> for SqlRockError {
    fn from(value: io::Error) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Column {
    name: String,
    data_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Table {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Statement {
    CreateTable {
        name: String,
        columns: Vec<Column>,
    },
    InsertInto {
        table: String,
        columns: Vec<String>,
        values: Vec<String>,
    },
    SelectAll {
        table: String,
    },
}

struct Database {
    root: PathBuf,
}

impl Database {
    fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn execute(&self, statement: Statement) -> Result<String> {
        match statement {
            Statement::CreateTable { name, columns } => self.create_table(&name, columns),
            Statement::InsertInto {
                table,
                columns,
                values,
            } => self.insert_into(&table, columns, values),
            Statement::SelectAll { table } => self.select_all(&table),
        }
    }

    fn create_table(&self, name: &str, columns: Vec<Column>) -> Result<String> {
        fs::create_dir_all(&self.root)?;

        let path = self.table_path(name)?;
        if path.exists() {
            return Err(SqlRockError::new(format!("table `{name}` already exists")));
        }

        let table = Table {
            name: name.to_string(),
            columns,
            rows: Vec::new(),
        };
        fs::write(path, serialize_table(&table))?;

        Ok(format!("created table `{name}`"))
    }

    fn insert_into(
        &self,
        table_name: &str,
        columns: Vec<String>,
        values: Vec<String>,
    ) -> Result<String> {
        if columns.len() != values.len() {
            return Err(SqlRockError::new(format!(
                "column count ({}) does not match value count ({})",
                columns.len(),
                values.len()
            )));
        }

        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let mut row = vec![String::new(); table.columns.len()];

        for (column_name, value) in columns.iter().zip(values.into_iter()) {
            let Some(index) = table
                .columns
                .iter()
                .position(|column| column.name.eq_ignore_ascii_case(column_name))
            else {
                return Err(SqlRockError::new(format!(
                    "unknown column `{column_name}` for table `{table_name}`"
                )));
            };
            row[index] = value;
        }

        table.rows.push(row);
        fs::write(path, serialize_table(&table))?;

        Ok(format!("inserted 1 row into `{table_name}`"))
    }

    fn select_all(&self, table_name: &str) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let table = parse_table_file(&fs::read_to_string(path)?)?;
        let mut lines = Vec::new();
        lines.push(
            table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        lines.extend(table.rows.iter().map(|row| row.join("\t")));

        Ok(lines.join("\n"))
    }

    fn table_path(&self, table_name: &str) -> Result<PathBuf> {
        validate_identifier(table_name)?;
        Ok(self.root.join(format!("{table_name}.table")))
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let mut data_dir = PathBuf::from("sql_rock_data");
    let mut sql_parts = Vec::new();

    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            let Some(value) = args.next() else {
                return Err(SqlRockError::new("--data-dir requires a path"));
            };
            data_dir = PathBuf::from(value);
        } else {
            sql_parts.push(arg);
        }
    }

    let sql = if sql_parts.is_empty() {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        sql_parts.join(" ")
    };

    if sql.trim().is_empty() {
        print_usage();
        return Ok(());
    }

    let database = Database::new(data_dir);
    for statement_sql in split_statements(&sql) {
        let statement = parse_statement(&statement_sql)?;
        println!("{}", database.execute(statement)?);
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  cargo run -- \"CREATE TABLE users (id INT, name TEXT);\"");
    println!("  cargo run -- \"INSERT INTO users (id, name) VALUES (1, 'Alice');\"");
    println!("  cargo run -- \"SELECT * FROM users;\"");
    println!("  cargo run -- --data-dir ./data \"CREATE TABLE users (id INT);\"");
}

fn parse_statement(sql: &str) -> Result<Statement> {
    let sql = sql.trim().trim_end_matches(';').trim();

    if starts_with_keyword(sql, "create table") {
        parse_create_table(sql)
    } else if starts_with_keyword(sql, "insert into") {
        parse_insert_into(sql)
    } else if starts_with_keyword(sql, "select") {
        parse_select(sql)
    } else {
        Err(SqlRockError::new(
            "unsupported SQL statement. supported: CREATE TABLE, INSERT INTO, SELECT * FROM",
        ))
    }
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
    let rest = strip_keyword(rest, "*")?.trim_start();
    let rest = strip_keyword(rest, "from")?.trim_start();
    let (table_name, trailing) = split_leading_identifier(rest)?;
    validate_identifier(table_name)?;

    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(format!(
            "unexpected trailing SQL: {}",
            trailing.trim()
        )));
    }

    Ok(Statement::SelectAll {
        table: table_name.to_string(),
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

fn serialize_table(table: &Table) -> String {
    let mut lines = Vec::new();
    lines.push(format!("table:{}", escape_field(&table.name)));
    lines.push(format!(
        "columns:{}",
        table
            .columns
            .iter()
            .map(|column| format!(
                "{}:{}",
                escape_field(&column.name),
                escape_field(&column.data_type)
            ))
            .collect::<Vec<_>>()
            .join("|")
    ));
    lines.push("rows:".to_string());
    lines.extend(table.rows.iter().map(|row| {
        row.iter()
            .map(|field| escape_field(field))
            .collect::<Vec<_>>()
            .join("|")
    }));
    lines.push(String::new());
    lines.join("\n")
}

fn parse_table_file(content: &str) -> Result<Table> {
    let mut lines = content.lines();
    let Some(table_line) = lines.next() else {
        return Err(SqlRockError::new("table file is empty"));
    };
    let Some(columns_line) = lines.next() else {
        return Err(SqlRockError::new("table file is missing columns"));
    };
    let Some(rows_marker) = lines.next() else {
        return Err(SqlRockError::new("table file is missing rows marker"));
    };

    let name = table_line
        .strip_prefix("table:")
        .ok_or_else(|| SqlRockError::new("invalid table line"))?;
    let columns = columns_line
        .strip_prefix("columns:")
        .ok_or_else(|| SqlRockError::new("invalid columns line"))?;
    if rows_marker != "rows:" {
        return Err(SqlRockError::new("invalid rows marker"));
    }

    let columns = if columns.is_empty() {
        Vec::new()
    } else {
        columns
            .split('|')
            .map(|item| {
                let Some((name, data_type)) = item.split_once(':') else {
                    return Err(SqlRockError::new("invalid column metadata"));
                };
                Ok(Column {
                    name: unescape_field(name)?,
                    data_type: unescape_field(data_type)?,
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }

        let row = line
            .split('|')
            .map(unescape_field)
            .collect::<Result<Vec<_>>>()?;
        if row.len() != columns.len() {
            return Err(SqlRockError::new("row width does not match table columns"));
        }
        rows.push(row);
    }

    Ok(Table {
        name: unescape_field(name)?,
        columns,
        rows,
    })
}

fn split_statements(sql: &str) -> Vec<String> {
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

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\p")
        .replace(':', "\\c")
        .replace('\n', "\\n")
}

fn unescape_field(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(SqlRockError::new("invalid escape sequence"));
        };
        match escaped {
            '\\' => result.push('\\'),
            'p' => result.push('|'),
            'c' => result.push(':'),
            'n' => result.push('\n'),
            other => {
                return Err(SqlRockError::new(format!(
                    "unknown escape sequence `\\{other}`"
                )));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_create_table() {
        let statement = parse_statement("CREATE TABLE users (id INT, name VARCHAR(255));").unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        data_type: "INT".to_string(),
                    },
                    Column {
                        name: "name".to_string(),
                        data_type: "VARCHAR(255)".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_insert_into() {
        let statement =
            parse_statement("INSERT INTO users (id, name) VALUES (1, 'Alice''s note');").unwrap();

        assert_eq!(
            statement,
            Statement::InsertInto {
                table: "users".to_string(),
                columns: vec!["id".to_string(), "name".to_string()],
                values: vec!["1".to_string(), "Alice's note".to_string()],
            }
        );
    }

    #[test]
    fn parses_select_all() {
        let statement = parse_statement("SELECT * FROM users;").unwrap();

        assert_eq!(
            statement,
            Statement::SelectAll {
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn create_table_insert_row_and_select_all() {
        let root = env::temp_dir().join(format!(
            "sql_rock_test_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let database = Database::new(&root);

        database
            .execute(parse_statement("CREATE TABLE users (id INT, name TEXT);").unwrap())
            .unwrap();
        database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (1, 'Alice');").unwrap())
            .unwrap();
        let output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        let table =
            parse_table_file(&fs::read_to_string(root.join("users.table")).unwrap()).unwrap();
        assert_eq!(table.rows, vec![vec!["1".to_string(), "Alice".to_string()]]);
        assert_eq!(output, "id\tname\n1\tAlice");

        fs::remove_dir_all(root).unwrap();
    }
}
