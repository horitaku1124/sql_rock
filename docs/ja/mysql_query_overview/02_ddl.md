# 2. DDL（Data Definition Language）

## データベース作成

```sql
CREATE DATABASE sample_db;
```

文字コード指定

```sql
CREATE DATABASE sample_db
CHARACTER SET utf8mb4
COLLATE utf8mb4_unicode_ci;
```

---

## データベース削除

```sql
DROP DATABASE sample_db;
```

---

## データベース選択

```sql
USE sample_db;
```

---

## テーブル作成

```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    email VARCHAR(255) UNIQUE,
    age INT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

テーブル全体の説明は、`COMMENT='...'` で任意に指定できる。

```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100) NOT NULL
) COMMENT='ユーザー情報を管理するテーブル';
```

このプロジェクトでは、次の範囲をサポートする。

- `CREATE TABLE table_name (...) COMMENT='comment'` 形式のテーブルコメント。
- コメントを省略した `CREATE TABLE table_name (...)`。
- テーブルファイルへのコメント保存。
- `SHOW CREATE TABLE` でのコメント表示。

カラムコメント、二重引用符で囲んだコメント、テーブル作成後のコメント変更には未対応。

### PRIMARY KEY / UNIQUE KEY

`PRIMARY KEY` はテーブル内の行を一意に識別するためのキーで、重複値を許可しない。`PRIMARY KEY` に指定したカラムは `NOT NULL` として扱われる。

`UNIQUE KEY` は、指定したカラムの値がテーブル内で重複しないことを保証するキーである。MySQL では `UNIQUE`、`UNIQUE KEY`、`UNIQUE INDEX` は同義として扱われる。

```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    email VARCHAR(255) UNIQUE KEY,
    name VARCHAR(100) NOT NULL
);
```

テーブル制約として指定することもできる。

```sql
CREATE TABLE users (
    id BIGINT AUTO_INCREMENT,
    email VARCHAR(255),
    name VARCHAR(100) NOT NULL,
    PRIMARY KEY (id),
    UNIQUE KEY unique_users_email (email)
);
```

このプロジェクトでは、次の範囲をサポートする。

- 単一カラムの `PRIMARY KEY`。
- 単一カラムの `UNIQUE` または `UNIQUE KEY`。
- カラム定義内の指定と、`PRIMARY KEY (column)` / `UNIQUE KEY name (column)` 形式のテーブル制約。
- `INSERT` と `UPDATE` における重複値のエラー。
- `PRIMARY KEY` カラムへの `NULL` 登録のエラー。

複合キー、キー名の保存、`UNIQUE INDEX`、インデックス構造の作成には未対応。

### AUTO_INCREMENT

MySQL では `AUTO_INCREMENT` 属性を使用すると、新しい行を追加する際に連番を自動生成できる。属性名は `AUTOINCREMENT` ではなく、アンダースコアを含む `AUTO_INCREMENT` と記述する。

```sql
CREATE TABLE users (
    id BIGINT NOT NULL AUTO_INCREMENT,
    name VARCHAR(100)
) AUTO_INCREMENT=100;

INSERT INTO users (name) VALUES ('Taro');
INSERT INTO users (name) VALUES ('Jiro');
```

上記の `id` には `100`、`101` が順に設定される。`AUTO_INCREMENT=100` を省略した場合は、初期値に `1` を指定したものとして扱う。

MySQL の主な仕様は次のとおり。

- テーブルごとに `AUTO_INCREMENT` を指定できるカラムは 1 つ。
- `AUTO_INCREMENT` を指定するカラムはインデックス対象にする。
- 値を省略すると連番が自動生成される。
- `0` を指定した場合も連番が生成される。ただし、MySQL では `NO_AUTO_VALUE_ON_ZERO` SQL モードを有効にすると挙動が変わる。
- `NOT NULL` のカラムに `NULL` を指定した場合も連番が生成される。
- 現在値より大きい値を明示的に登録すると、次回はその値に続く番号が生成される。
- `CREATE TABLE ... AUTO_INCREMENT=n` で初期値を指定できる。
- 負数は使用できない。
- 必要な最大値を格納できる、できるだけ小さい整数型を選択する。必要に応じて `UNSIGNED` を使用する。
- MySQL では浮動小数点型にも指定できるが、`FLOAT` と `DOUBLE` に対する指定は非推奨。
- `TRUNCATE TABLE` を実行すると採番値はリセットされる。

このプロジェクトでは、次の範囲をサポートする。

- 整数型のカラムに対する `AUTO_INCREMENT` 指定。
- `CREATE TABLE table_name (...) AUTO_INCREMENT=n` 形式の初期値指定。
- 初期値を省略した場合の `1` からの採番。
- 値を省略した場合、`NULL` を指定した場合、または `0` を指定した場合の連番生成。
- 大きい整数値を明示的に登録または更新した場合の、次回採番値の更新。
- `DELETE` 後の採番値の維持と、`TRUNCATE TABLE` 後の `1` へのリセット。
- テーブルファイルへの次回採番値の保存。

`LAST_INSERT_ID()`、`ALTER TABLE users AUTO_INCREMENT = 100`、SQL モードによる挙動変更には未対応。

参考: [MySQL 9.7 Reference Manual: Using AUTO_INCREMENT](https://dev.mysql.com/doc/refman/9.7/en/example-auto-increment.html)

参考: [MySQL 9.7 Reference Manual: Numeric Type Attributes](https://dev.mysql.com/doc/refman/9.7/en/numeric-type-attributes.html)

---

## テーブル変更

列追加

```sql
ALTER TABLE users
ADD COLUMN address VARCHAR(255);
```

列変更

```sql
ALTER TABLE users
MODIFY COLUMN age SMALLINT;
```

列名変更

```sql
ALTER TABLE users
CHANGE COLUMN name full_name VARCHAR(100);
```

---

## テーブル削除

```sql
DROP TABLE users;
```

---

## テーブル一覧

```sql
SHOW TABLES;
```

---

## テーブル定義確認

```sql
SHOW CREATE TABLE users;
```

または

```sql
DESCRIBE users;
```

---
