use crate::database::Database;
use crate::error::{Result, SqlRockError};
use crate::parser::{parse_statement, split_statements};
use std::env;
use std::io::{self, Read};
use std::path::PathBuf;

pub fn run() -> Result<()> {
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
    println!("  cargo run -- \"SELECT count(id) FROM users;\"");
    println!("  cargo run -- \"SELECT * FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"DESC users;\"");
    println!("  cargo run -- \"DROP TABLE users;\"");
    println!("  cargo run -- \"DELETE FROM users WHERE id = 1;\"");
    println!("  cargo run -- \"UPDATE users SET name = 'abc' WHERE id = 1;\"");
    println!("  cargo run -- --data-dir ./data \"CREATE TABLE users (id INT);\"");
}
