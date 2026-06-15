use sql_rock::dump::{DumpOptions, dump_directory};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, PartialEq, Eq)]
struct Config {
    data_dir: PathBuf,
    result_file: Option<PathBuf>,
    options: DumpOptions,
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
    let output =
        dump_directory(&config.data_dir, &config.options).map_err(|error| error.to_string())?;

    if let Some(path) = config.result_file {
        fs::write(&path, output)
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    } else {
        print!("{output}");
    }
    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Option<Config>, String> {
    let mut config = Config {
        data_dir: PathBuf::from("sql_rock_data"),
        result_file: None,
        options: DumpOptions::default(),
    };

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--data-dir" => {
                config.data_dir = PathBuf::from(next_value(&mut args, &argument)?);
            }
            "-r" | "--result-file" => {
                config.result_file = Some(PathBuf::from(next_value(&mut args, &argument)?));
            }
            "--no-data" => config.options.include_data = false,
            "--no-create-info" => config.options.include_create = false,
            "--add-drop-table" => config.options.add_drop_table = true,
            "--help" => {
                print_usage();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("sql_rock_dump {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            _ if argument.starts_with('-') => {
                return Err(format!("unsupported argument `{argument}`"));
            }
            _ => config.options.tables.push(argument),
        }
    }

    if !config.options.include_create && !config.options.include_data {
        return Err("both --no-data and --no-create-info were specified".to_string());
    }
    Ok(Some(config))
}

fn next_value(args: &mut impl Iterator<Item = String>, option: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn print_usage() {
    println!("sql_rock_dump {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage:");
    println!("  sql_rock_dump [OPTIONS] [TABLE ...]");
    println!();
    println!("Options:");
    println!("      --data-dir PATH     Table data directory (default: sql_rock_data)");
    println!("  -r, --result-file FILE  Write the dump to FILE");
    println!("      --no-data           Dump table definitions only");
    println!("      --no-create-info    Dump row data only");
    println!("      --add-drop-table    Add DROP TABLE before each table");
    println!("      --help              Print help");
    println!("  -V, --version           Print version");
}

#[cfg(test)]
mod tests {
    use super::{Config, parse_args};
    use sql_rock::dump::DumpOptions;
    use std::path::PathBuf;

    #[test]
    fn parses_dump_options_and_table_names() {
        let config = parse_args(
            [
                "--data-dir",
                "./data",
                "--result-file",
                "backup.sql",
                "--no-data",
                "--add-drop-table",
                "users",
                "orders",
            ]
            .into_iter()
            .map(String::from),
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            config,
            Config {
                data_dir: PathBuf::from("./data"),
                result_file: Some(PathBuf::from("backup.sql")),
                options: DumpOptions {
                    tables: vec!["users".to_string(), "orders".to_string()],
                    include_create: true,
                    include_data: false,
                    add_drop_table: true,
                },
            }
        );
    }

    #[test]
    fn rejects_an_empty_dump_selection() {
        let error = parse_args(
            ["--no-data", "--no-create-info"]
                .into_iter()
                .map(String::from),
        )
        .unwrap_err();
        assert_eq!(error, "both --no-data and --no-create-info were specified");
    }
}
