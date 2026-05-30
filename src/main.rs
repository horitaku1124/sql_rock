fn main() {
    if let Err(error) = sql_rock::cli::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
