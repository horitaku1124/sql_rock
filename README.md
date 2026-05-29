# sql_rock
minimum compatibility for MySQL


### How to run

```
cargo run
```

### Execute SQL

`CREATE TABLE` creates one table file under `sql_rock_data/`.
`INSERT INTO` appends row data to that table file.
`SELECT * FROM` reads all rows from the table file.

```
cargo run -- --data-dir ./data "CREATE TABLE users (id INT, name TEXT);"
cargo run -- --data-dir ./data "INSERT INTO users (id, name) VALUES (1, 'Alice');"
cargo run -- --data-dir ./data "SELECT * FROM users;"
```

Use `--data-dir` to choose a different storage directory.

```
cargo run -- --data-dir ./data "CREATE TABLE users (id INT, name TEXT);"
```
