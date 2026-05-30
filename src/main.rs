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
struct WhereClause {
    column: String,
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetClause {
    column: String,
    value: String,
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
        where_clause: Option<WhereClause>,
    },
    DescribeTable {
        table: String,
    },
    DropTable {
        table: String,
    },
    DeleteFrom {
        table: String,
        where_clause: WhereClause,
    },
    Update {
        table: String,
        set_clause: SetClause,
        where_clause: WhereClause,
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
            Statement::SelectAll {
                table,
                where_clause,
            } => self.select_all(&table, where_clause),
            Statement::DescribeTable { table } => self.describe_table(&table),
            Statement::DropTable { table } => self.drop_table(&table),
            Statement::DeleteFrom {
                table,
                where_clause,
            } => self.delete_from(&table, where_clause),
            Statement::Update {
                table,
                set_clause,
                where_clause,
            } => self.update_rows(&table, set_clause, where_clause),
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

    fn select_all(&self, table_name: &str, where_clause: Option<WhereClause>) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let table = parse_table_file(&fs::read_to_string(path)?)?;
        let where_index = match &where_clause {
            Some(where_clause) => Some(
                table
                    .columns
                    .iter()
                    .position(|column| column.name.eq_ignore_ascii_case(&where_clause.column))
                    .ok_or_else(|| {
                        SqlRockError::new(format!(
                            "unknown column `{}` for table `{table_name}`",
                            where_clause.column
                        ))
                    })?,
            ),
            None => None,
        };
        let mut lines = Vec::new();
        lines.push(
            table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        lines.extend(table.rows.iter().filter_map(|row| {
            let matches = match (&where_clause, where_index) {
                (Some(where_clause), Some(index)) => row.get(index) == Some(&where_clause.value),
                _ => true,
            };

            matches.then(|| row.join("\t"))
        }));

        Ok(lines.join("\n"))
    }

    fn describe_table(&self, table_name: &str) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let table = parse_table_file(&fs::read_to_string(path)?)?;
        let mut lines = Vec::new();
        lines.push("Field\tType".to_string());
        lines.extend(
            table
                .columns
                .iter()
                .map(|column| format!("{}\t{}", column.name, column.data_type)),
        );

        Ok(lines.join("\n"))
    }

    fn drop_table(&self, table_name: &str) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        fs::remove_file(path)?;
        Ok(format!("dropped table `{table_name}`"))
    }

    fn delete_from(&self, table_name: &str, where_clause: WhereClause) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let column_index = table
            .columns
            .iter()
            .position(|column| column.name.eq_ignore_ascii_case(&where_clause.column))
            .ok_or_else(|| {
                SqlRockError::new(format!(
                    "unknown column `{}` for table `{table_name}`",
                    where_clause.column
                ))
            })?;

        let original_len = table.rows.len();
        table
            .rows
            .retain(|row| row.get(column_index) != Some(&where_clause.value));
        let deleted_count = original_len - table.rows.len();

        fs::write(path, serialize_table(&table))?;
        Ok(format!(
            "deleted {deleted_count} row(s) from `{table_name}`"
        ))
    }

    fn update_rows(
        &self,
        table_name: &str,
        set_clause: SetClause,
        where_clause: WhereClause,
    ) -> Result<String> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let set_index = table
            .columns
            .iter()
            .position(|column| column.name.eq_ignore_ascii_case(&set_clause.column))
            .ok_or_else(|| {
                SqlRockError::new(format!(
                    "unknown column `{}` for table `{table_name}`",
                    set_clause.column
                ))
            })?;
        let where_index = table
            .columns
            .iter()
            .position(|column| column.name.eq_ignore_ascii_case(&where_clause.column))
            .ok_or_else(|| {
                SqlRockError::new(format!(
                    "unknown column `{}` for table `{table_name}`",
                    where_clause.column
                ))
            })?;

        let mut updated_count = 0;
        for row in &mut table.rows {
            if row.get(where_index) == Some(&where_clause.value) {
                row[set_index] = set_clause.value.clone();
                updated_count += 1;
            }
        }

        fs::write(path, serialize_table(&table))?;
        Ok(format!("updated {updated_count} row(s) in `{table_name}`"))
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
    println!("  cargo run -- \"SELECT * FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"DESC users;\"");
    println!("  cargo run -- \"DROP TABLE users;\"");
    println!("  cargo run -- \"DELETE FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"UPDATE users SET name = 'abc' WHERE id = 1;\"");
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
            "unsupported SQL statement. supported: CREATE TABLE, INSERT INTO, SELECT * FROM, DESC, DROP TABLE, DELETE FROM, UPDATE",
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "sql_rock_test_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            counter
        ))
    }

    fn setup_users_table() -> (PathBuf, Database) {
        let root = test_dir();
        let database = Database::new(&root);

        database
            .execute(parse_statement("CREATE TABLE users (id INT, name TEXT);").unwrap())
            .unwrap();
        database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (1, 'Alice');").unwrap())
            .unwrap();
        database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (2, 'Bob');").unwrap())
            .unwrap();

        (root, database)
    }

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
                where_clause: None,
            }
        );
    }

    #[test]
    fn parses_select_with_where() {
        let statement = parse_statement("SELECT * FROM users WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::SelectAll {
                table: "users".to_string(),
                where_clause: Some(WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                }),
            }
        );
    }

    #[test]
    fn parses_desc() {
        let statement = parse_statement("DESC users;").unwrap();

        assert_eq!(
            statement,
            Statement::DescribeTable {
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn parses_drop_table() {
        let statement = parse_statement("DROP TABLE users;").unwrap();

        assert_eq!(
            statement,
            Statement::DropTable {
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn parses_delete_from() {
        let statement = parse_statement("DELETE FROM users WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::DeleteFrom {
                table: "users".to_string(),
                where_clause: WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                },
            }
        );
    }

    #[test]
    fn parses_update() {
        let statement = parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::Update {
                table: "users".to_string(),
                set_clause: SetClause {
                    column: "name".to_string(),
                    value: "abc".to_string(),
                },
                where_clause: WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                },
            }
        );
    }

    #[test]
    fn splits_multiple_statements_with_strings() {
        let statements = split_statements(
            "INSERT INTO users (id, name) VALUES (1, 'Alice; note'); SELECT * FROM users;",
        );

        assert_eq!(
            statements,
            vec![
                "INSERT INTO users (id, name) VALUES (1, 'Alice; note')".to_string(),
                "SELECT * FROM users".to_string(),
            ]
        );
    }

    #[test]
    fn create_table_insert_row_and_select() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();
        let filtered_output = database
            .execute(parse_statement("SELECT * FROM users WHERE id = 1;").unwrap())
            .unwrap();
        let filtered_name_output = database
            .execute(parse_statement("SELECT * FROM users WHERE name = 'Bob';").unwrap())
            .unwrap();

        let table =
            parse_table_file(&fs::read_to_string(root.join("users.table")).unwrap()).unwrap();
        assert_eq!(
            table.rows,
            vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()]
            ]
        );
        assert_eq!(output, "id\tname\n1\tAlice\n2\tBob");
        assert_eq!(filtered_output, "id\tname\n1\tAlice");
        assert_eq!(filtered_name_output, "id\tname\n2\tBob");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn describe_table_returns_schema() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DESC users;").unwrap())
            .unwrap();

        assert_eq!(output, "Field\tType\nid\tINT\nname\tTEXT");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn drop_table_removes_table_file() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DROP TABLE users;").unwrap())
            .unwrap();
        let select_error = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap_err();

        assert_eq!(output, "dropped table `users`");
        assert_eq!(select_error.to_string(), "table `users` does not exist");
        assert!(!root.join("users.table").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_from_removes_matching_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DELETE FROM users WHERE id = 1;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "deleted 1 row(s) from `users`");
        assert_eq!(select_output, "id\tname\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_from_without_match_keeps_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DELETE FROM users WHERE id = 999;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "deleted 0 row(s) from `users`");
        assert_eq!(select_output, "id\tname\n1\tAlice\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_rows_updates_matching_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "updated 1 row(s) in `users`");
        assert_eq!(select_output, "id\tname\n1\tabc\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_rows_without_match_keeps_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 999;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "updated 0 row(s) in `users`");
        assert_eq!(select_output, "id\tname\n1\tAlice\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_where_with_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("SELECT * FROM users WHERE age = 20;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn insert_into_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("INSERT INTO users (age) VALUES (20);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_from_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn desc_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DESC users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn drop_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DROP TABLE users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn delete_from_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DELETE FROM users WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn delete_from_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("DELETE FROM users WHERE age = 20;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn update_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("UPDATE users SET age = 20 WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_existing_table_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("CREATE TABLE users (id INT, name TEXT);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` already exists");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_where_without_match_returns_header_only() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("SELECT * FROM users WHERE id = 999;").unwrap())
            .unwrap();

        assert_eq!(output, "id\tname");
        fs::remove_dir_all(root).unwrap();
    }
}
