# sql_rock

Rustで実装した、MySQL互換の学習用SQLデータベースです。

テーブル定義とデータは、データディレクトリ内のテーブル単位のファイルへ保存されます。

## CLI構成

| CLI | 用途 |
| --- | --- |
| `sql_rock` | SQLの直接実行とREPL |
| `sql_rock_server` | MySQLプロトコル互換サーバー |
| `sql_rock_client` | MySQLプロトコルで接続するクライアント |
| `sql_rock_dump` | テーブル定義とデータのSQLダンプ |

## ビルド

```sh
cargo build --bins
```

ビルドしたCLIは `target/debug/` に生成されます。

## sql_rock

SQLをコマンド引数から直接実行します。データディレクトリの初期値は
`sql_rock_data` です。

```sh
cargo run --bin sql_rock -- \
  --data-dir ./data \
  "CREATE TABLE users (id INT PRIMARY KEY AUTO_INCREMENT, name TEXT);"

cargo run --bin sql_rock -- \
  --data-dir ./data \
  "INSERT INTO users (name) VALUES ('Alice');"

cargo run --bin sql_rock -- \
  --data-dir ./data \
  "SELECT * FROM users;"
```

標準入力から複数のSQLを実行することもできます。

```sh
cargo run --bin sql_rock -- --data-dir ./data < backup.sql
```

### REPL

```sh
cargo run --bin sql_rock -- --data-dir ./data --repl
```

REPLではSQLのほか、`PREPARE`、`EXECUTE`、`DEALLOCATE PREPARE`を利用できます。
終了するには `exit`、`quit`、または `\q` を入力します。

## sql_rock_server

MySQLプロトコルでSQLを受け付けるサーバーです。

```sh
cargo run --bin sql_rock_server -- \
  --host 127.0.0.1 \
  --port 13307 \
  --data-dir ./data
```

接続情報は固定です。

| 項目 | 値 |
| --- | --- |
| ユーザー | `sqlrock` |
| パスワード | `sqlrock` |
| デフォルトポート | `13307` |

一般的なMySQLクライアントからも接続できます。

```sh
mysql \
  --host 127.0.0.1 \
  --port 13307 \
  --user sqlrock \
  --password=sqlrock
```

## sql_rock_client

MySQLプロトコル互換サーバーへ接続するクライアントです。

```sh
MYSQL_PWD=sqlrock cargo run --bin sql_rock_client -- \
  --host 127.0.0.1 \
  --port 13307 \
  --user sqlrock
```

SQLを1回だけ実行する場合は `--execute` を使います。

```sh
MYSQL_PWD=sqlrock cargo run --bin sql_rock_client -- \
  --host 127.0.0.1 \
  --port 13307 \
  --user sqlrock \
  --execute "SELECT NOW()"
```

主なオプション:

```text
-h, --host HOST
-P, --port PORT
-u, --user USER
-p, --password PASSWORD
-D, --database DATABASE
-e, --execute SQL
```

## sql_rock_dump

データディレクトリから、復元可能な `CREATE TABLE` と `INSERT INTO` を生成します。

```sh
cargo run --bin sql_rock_dump -- --data-dir ./data > backup.sql
```

特定のテーブルだけをダンプできます。

```sh
cargo run --bin sql_rock_dump -- --data-dir ./data users orders
```

ファイルへ直接出力する場合:

```sh
cargo run --bin sql_rock_dump -- \
  --data-dir ./data \
  --result-file backup.sql
```

主なオプション:

| オプション | 内容 |
| --- | --- |
| `--no-data` | テーブル定義のみ出力 |
| `--no-create-info` | 行データのみ出力 |
| `--add-drop-table` | 各テーブルの前に `DROP TABLE` を出力 |
| `-r, --result-file FILE` | ダンプをファイルへ出力 |

ダンプの復元には `sql_rock` を使用します。

```sh
cargo run --bin sql_rock -- --data-dir ./restored_data < backup.sql
```

## テスト

```sh
cargo fmt --check
cargo test
cargo build --bins
bash scripts/build-and-sh-run-check.sh
bash scripts/server-client-tests.sh
```

これらの検証はGitHub Actionsでも実行されます。
