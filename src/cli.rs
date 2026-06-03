use crate::database::Database;
use crate::error::{Result, SqlRockError};
use crate::parser::{parse_statement, split_statements};
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
    loop {
        print!("sql_rock> ");
        io::stdout().flush()?;

        let Some(line) = lines.next() else {
            println!();
            return Ok(());
        };
        let line = line?;
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if matches!(input.to_ascii_lowercase().as_str(), "exit" | "quit" | "\\q") {
            return Ok(());
        }

        if let Err(error) = execute_sql(database, input) {
            eprintln!("error: {error}");
        }
    }
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
