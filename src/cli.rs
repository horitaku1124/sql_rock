use crate::database::Database;
use crate::error::{Result, SqlRockError};
use crate::parser::{parse_statement, split_statements};
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;

pub fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let mut data_dir = PathBuf::from("sql_rock_data");
    let mut sql_parts = Vec::new();
    let mut force_repl = false;

    while let Some(arg) = args.next() {
        if arg == "--data-dir" {
            let Some(value) = args.next() else {
                return Err(SqlRockError::new("--data-dir requires a path"));
            };
            data_dir = PathBuf::from(value);
        } else if arg == "--repl" {
            force_repl = true;
        } else {
            sql_parts.push(arg);
        }
    }

    let database = Database::new(data_dir);
    if force_repl {
        return run_repl(&database);
    }

    if sql_parts.is_empty() && io::stdin().is_terminal() {
        return run_repl(&database);
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

    execute_sql(&database, &sql)
}

fn run_repl(database: &Database) -> Result<()> {
    println!("sql_rock REPL");
    println!("Enter SQL, or type `exit`, `quit`, or `\\q` to quit.");

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut buffer = String::new();
    let mut prepared_statements = HashMap::new();
    loop {
        if buffer.trim().is_empty() {
            print!("sql_rock> ");
        } else {
            print!("      ...> ");
        }
        io::stdout().flush()?;

        let Some(line) = lines.next() else {
            println!();
            return Ok(());
        };
        let line = line?;
        let input = line.trim();
        if input.is_empty() && buffer.trim().is_empty() {
            continue;
        }
        if buffer.trim().is_empty()
            && matches!(input.to_ascii_lowercase().as_str(), "exit" | "quit" | "\\q")
        {
            return Ok(());
        }

        buffer.push_str(&line);
        buffer.push('\n');

        if repl_input_is_ready(&buffer) {
            if let Err(error) = execute_repl_sql(database, &buffer, &mut prepared_statements) {
                eprintln!("error: {error}");
            }
            buffer.clear();
        }
    }
}

fn execute_repl_sql(
    database: &Database,
    sql: &str,
    prepared_statements: &mut HashMap<String, String>,
) -> Result<()> {
    let sql = strip_line_comments(sql);
    for statement_sql in split_statements(&sql) {
        execute_repl_statement(database, &statement_sql, prepared_statements)?;
    }
    Ok(())
}

fn execute_repl_statement(
    database: &Database,
    sql: &str,
    prepared_statements: &mut HashMap<String, String>,
) -> Result<()> {
    let sql = sql.trim();
    if starts_with_keyword(sql, "prepare") {
        let (name, statement) = parse_prepare(sql)?;
        prepared_statements.insert(name.to_ascii_lowercase(), statement);
        println!("prepared statement `{name}`");
        return Ok(());
    }

    if starts_with_keyword(sql, "execute") {
        let (name, parameters) = parse_execute(sql)?;
        let statement = prepared_statements
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| SqlRockError::new(format!("unknown prepared statement `{name}`")))?;
        let bound_sql = bind_parameters(statement, &parameters)?;
        return execute_sql(database, &bound_sql);
    }

    if starts_with_keyword(sql, "deallocate prepare") {
        let name = parse_deallocate(sql)?;
        if prepared_statements
            .remove(&name.to_ascii_lowercase())
            .is_none()
        {
            return Err(SqlRockError::new(format!(
                "unknown prepared statement `{name}`"
            )));
        }
        println!("deallocated prepared statement `{name}`");
        return Ok(());
    }

    execute_sql(database, sql)
}

fn parse_prepare(sql: &str) -> Result<(String, String)> {
    let rest = strip_keyword(sql, "prepare")?.trim_start();
    let (name, rest) = take_identifier(rest)?;
    let rest = strip_keyword(rest.trim_start(), "from")?.trim_start();
    let (statement, trailing) = take_quoted_string(rest)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new("unexpected trailing PREPARE syntax"));
    }
    if statement.trim().is_empty() {
        return Err(SqlRockError::new("PREPARE requires SQL text"));
    }
    Ok((name, statement))
}

fn parse_execute(sql: &str) -> Result<(String, Vec<String>)> {
    let rest = strip_keyword(sql, "execute")?.trim_start();
    let (name, rest) = take_identifier(rest)?;
    let rest = rest.trim_start();
    if rest.is_empty() {
        return Ok((name, Vec::new()));
    }

    let values = strip_keyword(rest, "using")?.trim_start();
    if values.is_empty() {
        return Err(SqlRockError::new("EXECUTE USING requires parameters"));
    }
    Ok((name, split_repl_parameters(values)?))
}

fn parse_deallocate(sql: &str) -> Result<String> {
    let rest = strip_keyword(sql, "deallocate prepare")?.trim_start();
    let (name, trailing) = take_identifier(rest)?;
    if !trailing.trim().is_empty() {
        return Err(SqlRockError::new(
            "unexpected trailing DEALLOCATE PREPARE syntax",
        ));
    }
    Ok(name)
}

fn bind_parameters(statement: &str, parameters: &[String]) -> Result<String> {
    let mut result = String::new();
    let mut parameter_index = 0;
    let mut in_string = false;
    let mut chars = statement.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            result.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                result.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if ch == '?' && !in_string {
            let parameter = parameters.get(parameter_index).ok_or_else(|| {
                SqlRockError::new(format!(
                    "prepared statement requires more than {} parameter(s)",
                    parameters.len()
                ))
            })?;
            result.push_str(parameter);
            parameter_index += 1;
        } else {
            result.push(ch);
        }
    }

    if parameter_index != parameters.len() {
        return Err(SqlRockError::new(format!(
            "prepared statement requires {parameter_index} parameter(s), but {} were supplied",
            parameters.len()
        )));
    }
    Ok(result)
}

fn split_repl_parameters(input: &str) -> Result<Vec<String>> {
    let mut parameters = Vec::new();
    let mut current = String::new();
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
        } else if ch == ',' && !in_string {
            push_repl_parameter(&mut parameters, &mut current)?;
        } else {
            current.push(ch);
        }
    }
    if in_string {
        return Err(SqlRockError::new("unterminated EXECUTE parameter"));
    }
    push_repl_parameter(&mut parameters, &mut current)?;
    Ok(parameters)
}

fn push_repl_parameter(parameters: &mut Vec<String>, current: &mut String) -> Result<()> {
    let parameter = current.trim();
    if parameter.is_empty() {
        return Err(SqlRockError::new("EXECUTE contains an empty parameter"));
    }
    validate_repl_parameter(parameter)?;
    parameters.push(parameter.to_string());
    current.clear();
    Ok(())
}

fn validate_repl_parameter(parameter: &str) -> Result<()> {
    let is_keyword = ["null", "now()", "today()"]
        .iter()
        .any(|value| parameter.eq_ignore_ascii_case(value));
    let is_number = parameter.parse::<f64>().is_ok();
    let is_string = if parameter.starts_with('\'') {
        take_quoted_string(parameter).is_ok_and(|(_, trailing)| trailing.trim().is_empty())
    } else {
        false
    };

    if is_keyword || is_number || is_string {
        Ok(())
    } else {
        Err(SqlRockError::new(format!(
            "unsupported EXECUTE parameter `{parameter}`"
        )))
    }
}

fn take_identifier(input: &str) -> Result<(String, &str)> {
    let input = input.trim_start();
    let end = input
        .char_indices()
        .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    if end == 0
        || !input
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        return Err(SqlRockError::new("expected prepared statement name"));
    }
    Ok((input[..end].to_string(), &input[end..]))
}

fn take_quoted_string(input: &str) -> Result<(String, &str)> {
    let Some(rest) = input.strip_prefix('\'') else {
        return Err(SqlRockError::new("PREPARE FROM requires quoted SQL"));
    };
    let mut value = String::new();
    let mut chars = rest.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '\'' {
            if chars.peek().is_some_and(|(_, next)| *next == '\'') {
                chars.next();
                value.push('\'');
            } else {
                return Ok((value, &rest[index + ch.len_utf8()..]));
            }
        } else {
            value.push(ch);
        }
    }
    Err(SqlRockError::new("unterminated PREPARE SQL string"))
}

fn strip_line_comments(input: &str) -> String {
    let mut result = String::new();
    let mut in_string = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            result.push(ch);
            if in_string && chars.peek() == Some(&'\'') {
                result.push(chars.next().expect("peeked character exists"));
            } else {
                in_string = !in_string;
            }
        } else if ch == '-' && !in_string && chars.peek() == Some(&'-') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    result.push('\n');
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

fn starts_with_keyword(input: &str, keyword: &str) -> bool {
    input
        .get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
        && input
            .get(keyword.len()..)
            .and_then(|rest| rest.chars().next())
            .is_none_or(|ch| ch.is_whitespace())
}

fn strip_keyword<'a>(input: &'a str, keyword: &str) -> Result<&'a str> {
    if starts_with_keyword(input, keyword) {
        Ok(&input[keyword.len()..])
    } else {
        Err(SqlRockError::new(format!("expected keyword `{keyword}`")))
    }
}

fn repl_input_is_ready(input: &str) -> bool {
    let mut in_string = false;
    let mut last_non_whitespace_is_semicolon = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if in_string && chars.peek() == Some(&'\'') {
                chars.next();
            } else {
                in_string = !in_string;
            }
            last_non_whitespace_is_semicolon = false;
        } else if !in_string && !ch.is_whitespace() {
            last_non_whitespace_is_semicolon = ch == ';';
        } else if in_string {
            last_non_whitespace_is_semicolon = false;
        }
    }

    !in_string && last_non_whitespace_is_semicolon
}

fn execute_sql(database: &Database, sql: &str) -> Result<()> {
    for statement_sql in split_statements(sql) {
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
    println!("  cargo run -- \"SELECT count(id) FROM users;\"");
    println!("  cargo run -- \"SELECT * FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"DESC users;\"");
    println!("  cargo run -- \"DROP TABLE users;\"");
    println!("  cargo run -- \"DELETE FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"UPDATE users SET name = 'abc' WHERE id = 1;\"");
    println!("  cargo run -- --data-dir ./data \"CREATE TABLE users (id INT);\"");
    println!("  cargo run -- --repl");
}

#[cfg(test)]
mod tests {
    use super::{
        bind_parameters, parse_deallocate, parse_execute, parse_prepare, repl_input_is_ready,
        strip_line_comments,
    };

    #[test]
    fn repl_waits_until_statement_terminator() {
        assert!(!repl_input_is_ready("CREATE TABLE users (\n"));
        assert!(!repl_input_is_ready(
            "CREATE TABLE users (\n    id INT,\n    name TEXT\n)"
        ));
        assert!(repl_input_is_ready(
            "CREATE TABLE users (\n    id INT,\n    name TEXT\n);\n"
        ));
    }

    #[test]
    fn repl_ignores_semicolon_inside_string() {
        assert!(!repl_input_is_ready(
            "INSERT INTO users (name) VALUES ('a;b')"
        ));
        assert!(repl_input_is_ready(
            "INSERT INTO users (name) VALUES ('a;b');"
        ));
    }

    #[test]
    fn repl_waits_for_closed_string() {
        assert!(!repl_input_is_ready(
            "INSERT INTO users (name) VALUES ('abc);"
        ));
        assert!(repl_input_is_ready(
            "INSERT INTO users (name) VALUES ('Alice''s');"
        ));
    }

    #[test]
    fn parses_repl_prepared_statement_commands() {
        assert_eq!(
            parse_prepare("PREPARE stmt FROM 'SELECT * FROM users WHERE id = ?'").unwrap(),
            (
                "stmt".to_string(),
                "SELECT * FROM users WHERE id = ?".to_string()
            )
        );
        assert_eq!(
            parse_execute("EXECUTE stmt USING 123, 'Alice'").unwrap(),
            (
                "stmt".to_string(),
                vec!["123".to_string(), "'Alice'".to_string()]
            )
        );
        assert_eq!(parse_deallocate("DEALLOCATE PREPARE stmt").unwrap(), "stmt");
    }

    #[test]
    fn binds_only_placeholders_outside_strings() {
        assert_eq!(
            bind_parameters(
                "SELECT * FROM users WHERE id = ? AND name = '?'",
                &["123".to_string()]
            )
            .unwrap(),
            "SELECT * FROM users WHERE id = 123 AND name = '?'"
        );
    }

    #[test]
    fn rejects_prepared_statement_parameter_count_mismatch() {
        assert!(
            bind_parameters(
                "SELECT * FROM users WHERE id = ? AND name = ?",
                &["123".to_string()]
            )
            .is_err()
        );
        assert!(
            bind_parameters(
                "SELECT * FROM users WHERE id = ?",
                &["123".to_string(), "456".to_string()]
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_non_literal_execute_parameters() {
        assert!(parse_execute("EXECUTE stmt USING 123 OR 1 = 1").is_err());
        assert!(parse_execute("EXECUTE stmt USING 'Alice', NULL, now()").is_ok());
    }

    #[test]
    fn removes_repl_line_comments_outside_strings() {
        assert_eq!(
            strip_line_comments(
                "-- prepare query\nPREPARE stmt FROM 'SELECT ''--'' FROM users WHERE id = ?';"
            ),
            "\nPREPARE stmt FROM 'SELECT ''--'' FROM users WHERE id = ?';"
        );
    }
}
