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
`WHERE` supports simple equality filters like `WHERE id = 1`.
`DESC` shows the table schema.
`DROP TABLE` deletes the table file.
`DELETE FROM` deletes rows that match a `WHERE` condition.
`UPDATE` updates rows that match a `WHERE` condition.

```
cargo run -- --data-dir ./data "CREATE TABLE users (id INT, name TEXT);"
cargo run -- --data-dir ./data "INSERT INTO users (id, name) VALUES (1, 'Alice');"
cargo run -- --data-dir ./data "SELECT * FROM users;"
cargo run -- --data-dir ./data "SELECT * FROM users WHERE id = 1;"
cargo run -- --data-dir ./data "DESC users;"
cargo run -- --data-dir ./data "DROP TABLE users;"
cargo run -- --data-dir ./data "DELETE FROM users WHERE id = 1;"
cargo run -- --data-dir ./data "UPDATE users SET name = 'abc' WHERE id = 1;"
```

Use `--data-dir` to choose a different storage directory.

```
cargo run -- --data-dir ./data "CREATE TABLE users (id INT, name TEXT);"
```
