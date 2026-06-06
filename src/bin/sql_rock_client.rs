use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: impl Iterator<Item = String>) -> Result<(), String> {
    match args.next().as_deref() {
        None | Some("-h" | "--help") => {
            print_usage();
            Ok(())
        }
        Some("-V" | "--version") => {
            println!("sql_rock_client {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(argument) => Err(format!("unsupported argument `{argument}`")),
    }
}

fn print_usage() {
    println!("sql_rock_client {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage:");
    println!("  sql_rock_client [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -h, --help       Print help");
    println!("  -V, --version    Print version");
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn accepts_help_and_version_options() {
        assert!(run(["--help".to_string()].into_iter()).is_ok());
        assert!(run(["--version".to_string()].into_iter()).is_ok());
    }

    #[test]
    fn rejects_unsupported_arguments() {
        let error = run(["--unknown".to_string()].into_iter()).unwrap_err();
        assert_eq!(error, "unsupported argument `--unknown`");
    }
}
