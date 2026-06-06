#[path = "sql_rock_server/protocol.rs"]
mod protocol;

use protocol::serve_connection;
use sql_rock::database::Database;
use std::env;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::thread;

const USERNAME: &str = "sqlrock";
const PASSWORD: &str = "sqlrock";

struct Config {
    host: String,
    port: u16,
    data_dir: PathBuf,
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
    let listener = TcpListener::bind((config.host.as_str(), config.port))
        .map_err(|error| format!("cannot listen on {}:{}: {error}", config.host, config.port))?;
    let database = Arc::new(Mutex::new(Database::new(config.data_dir)));

    println!(
        "sql_rock_server listening on {}:{} (user: {USERNAME})",
        config.host, config.port
    );

    for connection in listener.incoming() {
        match connection {
            Ok(stream) => {
                let database = Arc::clone(&database);
                thread::spawn(move || {
                    if let Err(error) = serve_connection(stream, &database, USERNAME, PASSWORD) {
                        eprintln!("connection error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }
    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Option<Config>, String> {
    let mut config = Config {
        host: "127.0.0.1".to_string(),
        port: 13307,
        data_dir: PathBuf::from("sql_rock_data"),
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
            "--data-dir" => config.data_dir = PathBuf::from(next_value(&mut args, &argument)?),
            "--help" => {
                print_usage();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("sql_rock_server {}", env!("CARGO_PKG_VERSION"));
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

fn print_usage() {
    println!("sql_rock_server {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage:");
    println!("  sql_rock_server [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --host HOST      Listen host (default: 127.0.0.1)");
    println!("  -P, --port PORT      Listen port (default: 13307)");
    println!("      --data-dir PATH  Table data directory (default: sql_rock_data)");
    println!("      --help           Print help");
    println!("  -V, --version        Print version");
    println!();
    println!("Fixed credentials:");
    println!("  user: {USERNAME}");
    println!("  password: {PASSWORD}");
}

#[cfg(test)]
mod tests {
    use super::parse_args;
    use std::path::PathBuf;

    #[test]
    fn parses_server_options() {
        let config = parse_args(
            [
                "--host",
                "0.0.0.0",
                "--port",
                "14000",
                "--data-dir",
                "./data",
            ]
            .into_iter()
            .map(String::from),
        )
        .unwrap()
        .unwrap();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 14000);
        assert_eq!(config.data_dir, PathBuf::from("./data"));
    }
}
