#[path = "sql_rock_client/mysql.rs"]
mod mysql;

use mysql::{MySqlClient, QueryResult};
use std::env;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

#[derive(Debug, PartialEq, Eq)]
struct Config {
    host: String,
    port: u16,
    user: String,
    password: String,
    database: Option<String>,
    execute: Option<String>,
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let Some(config) = parse_args(args)? else {
        return Ok(());
    };

    let mut client = MySqlClient::connect(
        &config.host,
        config.port,
        &config.user,
        &config.password,
        config.database.as_deref(),
    )?;

    if let Some(sql) = config.execute {
        print_result(client.query(&sql)?);
        return Ok(());
    }

    run_repl(&mut client)
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Option<Config>, String> {
    let mut config = Config {
        host: "127.0.0.1".to_string(),
        port: 3306,
        user: "root".to_string(),
        password: env::var("MYSQL_PWD").unwrap_or_default(),
        database: None,
        execute: None,
    };

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--host" => config.host = next_value(&mut args, &argument)?,
            "-P" | "--port" => {
                let value = next_value(&mut args, &argument)?;
                config.port = value
                    .parse()
                    .map_err(|_| format!("invalid port `{value}`"))?;
            }
            "-u" | "--user" => config.user = next_value(&mut args, &argument)?,
            "-p" | "--password" => config.password = next_value(&mut args, &argument)?,
            "-D" | "--database" => config.database = Some(next_value(&mut args, &argument)?),
            "-e" | "--execute" => config.execute = Some(next_value(&mut args, &argument)?),
            "--help" => {
                print_usage();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("sql_rock_client {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ => return Err(format!("unsupported argument `{argument}`")),
        }
    }

    Ok(Some(config))
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn run_repl(client: &mut MySqlClient) -> Result<(), String> {
    let interactive = io::stdin().is_terminal();
    if interactive {
        println!("Connected to MySQL. End statements with `;`. Type `quit` to exit.");
    }

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let mut buffer = String::new();
    loop {
        if interactive {
            let prompt = if buffer.is_empty() {
                "mysql> "
            } else {
                "    -> "
            };
            print!("{prompt}");
            io::stdout().flush().map_err(|error| error.to_string())?;
        }

        let Some(line) = lines.next() else {
            break;
        };
        let line = line.map_err(|error| error.to_string())?;
        let trimmed = line.trim();
        if buffer.is_empty() && matches!(trimmed.to_ascii_lowercase().as_str(), "quit" | "exit") {
            break;
        }

        buffer.push_str(&line);
        buffer.push('\n');
        if statement_is_complete(&buffer) {
            let sql = buffer.trim().trim_end_matches(';').trim();
            if !sql.is_empty() {
                match client.query(sql) {
                    Ok(result) => print_result(result),
                    Err(error) => eprintln!("error: {error}"),
                }
            }
            buffer.clear();
        }
    }
    Ok(())
}

fn statement_is_complete(sql: &str) -> bool {
    let mut in_string = false;
    let mut last_non_whitespace = None;
    let mut chars = sql.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\'' {
            if in_string && chars.peek() == Some(&'\'') {
                chars.next();
            } else {
                in_string = !in_string;
            }
        }
        if !ch.is_whitespace() {
            last_non_whitespace = Some(ch);
        }
    }
    !in_string && last_non_whitespace == Some(';')
}

fn print_result(result: QueryResult) {
    match result {
        QueryResult::Command {
            affected_rows,
            last_insert_id,
        } => {
            if last_insert_id == 0 {
                println!("Query OK, {affected_rows} row(s) affected");
            } else {
                println!("Query OK, {affected_rows} row(s) affected, insert id {last_insert_id}");
            }
        }
        QueryResult::Rows { columns, rows } => {
            println!("{}", columns.join("\t"));
            for row in rows {
                println!(
                    "{}",
                    row.into_iter()
                        .map(|value| value.unwrap_or_else(|| "NULL".to_string()))
                        .collect::<Vec<_>>()
                        .join("\t")
                );
            }
        }
    }
}

fn print_usage() {
    println!("sql_rock_client {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage:");
    println!("  sql_rock_client [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --host HOST          Server host (default: 127.0.0.1)");
    println!("  -P, --port PORT          Server port (default: 3306)");
    println!("  -u, --user USER          MySQL user (default: root)");
    println!("  -p, --password PASSWORD  MySQL password (or MYSQL_PWD)");
    println!("  -D, --database DATABASE  Initial database");
    println!("  -e, --execute SQL        Execute SQL and exit");
    println!("      --help               Print help");
    println!("  -V, --version            Print version");
}

#[cfg(test)]
mod tests {
    use super::{Config, parse_args, statement_is_complete};

    #[test]
    fn parses_connection_options() {
        let config = parse_args(
            [
                "--host",
                "db.example",
                "--port",
                "3307",
                "--user",
                "alice",
                "--password",
                "secret",
                "--database",
                "app",
                "--execute",
                "SELECT 1",
            ]
            .into_iter()
            .map(String::from),
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            config,
            Config {
                host: "db.example".to_string(),
                port: 3307,
                user: "alice".to_string(),
                password: "secret".to_string(),
                database: Some("app".to_string()),
                execute: Some("SELECT 1".to_string()),
            }
        );
    }

    #[test]
    fn recognizes_complete_statements() {
        assert!(!statement_is_complete("SELECT ';'"));
        assert!(statement_is_complete("SELECT ';';"));
    }
}
