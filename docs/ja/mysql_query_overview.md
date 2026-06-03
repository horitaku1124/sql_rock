# MySQL クエリ仕様まとめ

## 1. SQLの基本構文

MySQLはSQL（Structured Query Language）を使用してデータベースを操作します。

主な分類：

| 分類 | 内容 |
|--------|--------|
| DDL | テーブルやデータベース定義 |
| DML | データ操作 |
| DQL | データ検索 |
| DCL | 権限管理 |
| TCL | トランザクション制御 |

---

## 1.1 データ型

MySQL のデータ型は、数値型、日付・時刻型、文字列・バイト列型、空間型、`JSON` 型に大別される。

### 1.1.1 型指定で使う記号

| 記号 | 内容 |
|---|---|
| `M` | 整数型では最大表示幅、浮動小数点型・固定小数点型では格納できる数字の総桁数、文字列型では最大長を表す。整数型の表示幅は値の範囲を制限しない。 |
| `D` | 浮動小数点型・固定小数点型で、小数点以下の桁数を表す。 |
| `fsp` | `TIME`、`DATETIME`、`TIMESTAMP` で秒の小数部の桁数を表す。指定可能な値は `0` から `6` で、省略時は `0`。 |

数値型には、符号なしの値だけを扱う `UNSIGNED` 属性を指定できる。`ZEROFILL` 属性と整数型の表示幅は非推奨である。

### 1.1.2 数値型

#### 整数型

整数を正確に格納する。必要な値の範囲に応じて型を選択する。

| 型 | 内容 |
|---|---|
| `TINYINT` | 小さい範囲の整数 |
| `SMALLINT` | `TINYINT` より広い範囲の整数 |
| `MEDIUMINT` | 中程度の範囲の整数 |
| `INT`, `INTEGER` | 一般的な整数。`INT` と `INTEGER` は同義 |
| `BIGINT` | 大きい範囲の整数 |

#### 固定小数点型

| 型 | 内容 |
|---|---|
| `DECIMAL(M,D)`, `NUMERIC(M,D)` | 小数を正確に格納する。金額など、丸め誤差を避けたい値に適する。`DEC` と `FIXED` も `DECIMAL` の同義語。 |

`M` と `D` を省略した場合、`DECIMAL` のデフォルトは `DECIMAL(10,0)` となる。

#### 浮動小数点型

| 型 | 内容 |
|---|---|
| `FLOAT` | 単精度の近似値 |
| `DOUBLE`, `DOUBLE PRECISION` | 倍精度の近似値。`DOUBLE` と `DOUBLE PRECISION` は同義 |
| `REAL` | 通常は `DOUBLE PRECISION` の同義。`REAL_AS_FLOAT` SQL モードでは `FLOAT` の同義 |

#### ビット値型

| 型 | 内容 |
|---|---|
| `BIT(M)` | ビット値を格納する。`M` は `1` から `64` で、省略時は `1`。 |

### 1.1.3 日付・時刻型

| 型 | 内容 |
|---|---|
| `DATE` | 日付 |
| `TIME(fsp)` | 時刻または経過時間 |
| `DATETIME(fsp)` | 日付と時刻 |
| `TIMESTAMP(fsp)` | タイムスタンプ。`DATETIME` と同様に自動初期化・自動更新を設定できる。 |
| `YEAR` | 年 |

入力値には複数の形式を利用できるが、日付部分は年、月、日の順で指定する。無効な値を扱う際の挙動は SQL モードにも依存する。

### 1.1.4 文字列・バイト列型

| 型 | 内容 |
|---|---|
| `CHAR(M)` | 固定長の文字列 |
| `VARCHAR(M)` | 可変長の文字列 |
| `BINARY(M)` | 固定長のバイト列 |
| `VARBINARY(M)` | 可変長のバイト列 |
| `TINYBLOB`, `BLOB`, `MEDIUMBLOB`, `LONGBLOB` | バイナリデータ。型ごとに格納可能な最大長が異なる。 |
| `TINYTEXT`, `TEXT`, `MEDIUMTEXT`, `LONGTEXT` | テキストデータ。型ごとに格納可能な最大長が異なる。 |
| `ENUM` | 定義済みの候補から 1 つの値を格納する。 |
| `SET` | 定義済みの候補から 0 個以上の値を組み合わせて格納する。 |

非バイナリ文字列の長さは文字数、バイナリ文字列の長さはバイト数として扱う。文字列型には文字セットを指定する `CHARACTER SET` と、照合順序を指定する `COLLATE` を設定できる。

#### VECTOR 型

| 型 | 内容 |
|---|---|
| `VECTOR(N)` | `N` 個の単精度浮動小数点値を格納するベクトル。`N` のデフォルトは `2048`、最大値は `16383`。 |

`VECTOR` は主キー、外部キー、ユニークキー、パーティションキーには利用できない。別の `VECTOR` との等価比較は可能だが、それ以外の比較には利用できない。

### 1.1.5 空間型

空間型は OpenGIS のクラスに対応し、位置や形状を表す値を格納する。

| 分類 | 型 | 内容 |
|---|---|---|
| 単一値 | `GEOMETRY` | 任意の種類の空間値 |
| 単一値 | `POINT` | 点 |
| 単一値 | `LINESTRING` | 線 |
| 単一値 | `POLYGON` | 多角形 |
| コレクション | `MULTIPOINT` | 複数の点 |
| コレクション | `MULTILINESTRING` | 複数の線 |
| コレクション | `MULTIPOLYGON` | 複数の多角形 |
| コレクション | `GEOMETRYCOLLECTION` | 任意の種類の空間値のコレクション |

空間型のカラムには、格納する値の空間参照系を制限する `SRID` 属性を設定できる。`SPATIAL` インデックスを利用する場合は、特定の `SRID` と `NOT NULL` を指定する。

### 1.1.6 JSON 型

| 型 | 内容 |
|---|---|
| `JSON` | JSON ドキュメントを格納する。 |

`JSON` 型は、格納時にドキュメントを検証し、不正な JSON をエラーにする。値は要素へ効率よくアクセスできる内部形式に変換される。

`JSON` カラム自体には直接インデックスを作成できない。JSON からスカラー値を取り出す生成カラムにインデックスを作成するか、JSON 配列に対するマルチバリューインデックスを利用する。

### 1.1.7 型選択の指針

- 格納する値を正確に表現できる、最小限の大きさの型を選択する。
- 固定長と可変長のどちらを使うかは、データの性質と格納効率を踏まえて選択する。
- `NULL` が不要なカラムには `NOT NULL` を指定する。
- 型の選択による性能差は、実際のデータと利用環境で検証する。

### 1.1.8 参考資料

- [MySQL 9.7 Reference Manual: Data Types](https://dev.mysql.com/doc/refman/9.7/en/data-types.html)
- [Numeric Data Types](https://dev.mysql.com/doc/refman/9.7/en/numeric-types.html)
- [Date and Time Data Types](https://dev.mysql.com/doc/refman/9.7/en/date-and-time-types.html)
- [String Data Types](https://dev.mysql.com/doc/refman/9.7/en/string-types.html)
- [The VECTOR Type](https://dev.mysql.com/doc/refman/9.7/en/vector.html)
- [Spatial Data Types](https://dev.mysql.com/doc/refman/9.7/en/spatial-type-overview.html)
- [The JSON Data Type](https://dev.mysql.com/doc/refman/9.7/en/json.html)

---

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

### AUTO_INCREMENT

MySQL では `AUTO_INCREMENT` 属性を使用すると、新しい行を追加する際に連番を自動生成できる。属性名は `AUTOINCREMENT` ではなく、アンダースコアを含む `AUTO_INCREMENT` と記述する。

```sql
CREATE TABLE users (
    id BIGINT NOT NULL AUTO_INCREMENT,
    name VARCHAR(100)
);

INSERT INTO users (name) VALUES ('Taro');
INSERT INTO users (name) VALUES ('Jiro');
```

上記の `id` には `1`、`2` が順に設定される。

MySQL の主な仕様は次のとおり。

- テーブルごとに `AUTO_INCREMENT` を指定できるカラムは 1 つ。
- `AUTO_INCREMENT` を指定するカラムはインデックス対象にする。
- 値を省略すると連番が自動生成される。
- `0` を指定した場合も連番が生成される。ただし、MySQL では `NO_AUTO_VALUE_ON_ZERO` SQL モードを有効にすると挙動が変わる。
- `NOT NULL` のカラムに `NULL` を指定した場合も連番が生成される。
- 現在値より大きい値を明示的に登録すると、次回はその値に続く番号が生成される。
- 負数は使用できない。
- 必要な最大値を格納できる、できるだけ小さい整数型を選択する。必要に応じて `UNSIGNED` を使用する。
- MySQL では浮動小数点型にも指定できるが、`FLOAT` と `DOUBLE` に対する指定は非推奨。
- `TRUNCATE TABLE` を実行すると採番値はリセットされる。

このプロジェクトでは、次の範囲をサポートする。

- 整数型のカラムに対する `AUTO_INCREMENT` 指定。
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

# 3. DML（Data Manipulation Language）

## INSERT

### 単一行

```sql
INSERT INTO users (
    name,
    email,
    age
)
VALUES (
    'Taro',
    'taro@example.com',
    20
);
```

---

### 複数行

```sql
INSERT INTO users (
    name,
    email,
    age
)
VALUES
('Taro', 'taro@example.com', 20),
('Jiro', 'jiro@example.com', 25),
('Saburo', 'saburo@example.com', 30);
```

---

### SELECT結果をINSERT

```sql
INSERT INTO archive_users
SELECT *
FROM users
WHERE age > 60;
```

---

## UPDATE

```sql
UPDATE users
SET age = 21
WHERE id = 1;
```

複数列

```sql
UPDATE users
SET
    age = 22,
    email = 'new@example.com'
WHERE id = 1;
```

---

## DELETE

```sql
DELETE FROM users
WHERE id = 1;
```

全件削除

```sql
DELETE FROM users;
```

---

## TRUNCATE

高速削除

```sql
TRUNCATE TABLE users;
```

特徴

- AUTO_INCREMENTがリセットされる
- ログ量が少ない
- 高速

---

# 4. DQL（Data Query Language）

## SELECT

全件取得

```sql
SELECT *
FROM users;
```

特定列

```sql
SELECT
    id,
    name
FROM users;
```

---

## DISTINCT

重複除外

```sql
SELECT DISTINCT age
FROM users;
```

---

## WHERE

```sql
SELECT *
FROM users
WHERE age >= 20;
```

---

### 比較演算子

| 演算子 | 意味 |
|----------|----------|
| = | 等しい |
| <> | 等しくない |
| != | 等しくない |
| > | より大きい |
| < | より小さい |
| >= | 以上 |
| <= | 以下 |

---

### BETWEEN

```sql
SELECT *
FROM users
WHERE age BETWEEN 20 AND 30;
```

---

### IN

```sql
SELECT *
FROM users
WHERE age IN (20, 25, 30);
```

---

### LIKE

```sql
SELECT *
FROM users
WHERE name LIKE 'Ta%';
```

| パターン | 意味 |
|------------|------------|
| % | 任意文字列 |
| _ | 任意1文字 |

---

### NULL判定

```sql
SELECT *
FROM users
WHERE email IS NULL;
```

```sql
SELECT *
FROM users
WHERE email IS NOT NULL;
```

---

## ORDER BY

昇順

```sql
SELECT *
FROM users
ORDER BY age ASC;
```

降順

```sql
SELECT *
FROM users
ORDER BY age DESC;
```

複数列

```sql
SELECT *
FROM users
ORDER BY age DESC, name ASC;
```

---

## LIMIT

```sql
SELECT *
FROM users
LIMIT 10;
```

ページング

```sql
SELECT *
FROM users
LIMIT 20 OFFSET 40;
```

---

# 5. 集約関数

## COUNT

```sql
SELECT COUNT(*)
FROM users;
```

---

## SUM

```sql
SELECT SUM(amount)
FROM orders;
```

---

## AVG

```sql
SELECT AVG(age)
FROM users;
```

---

## MAX

```sql
SELECT MAX(age)
FROM users;
```

---

## MIN

```sql
SELECT MIN(age)
FROM users;
```

---

# 6. GROUP BY

```sql
SELECT
    age,
    COUNT(*)
FROM users
GROUP BY age;
```

---

## HAVING

```sql
SELECT
    age,
    COUNT(*)
FROM users
GROUP BY age
HAVING COUNT(*) >= 5;
```

---

# 7. JOIN

## INNER JOIN

```sql
SELECT
    u.name,
    o.amount
FROM users u
INNER JOIN orders o
ON u.id = o.user_id;
```

---

## LEFT JOIN

```sql
SELECT *
FROM users u
LEFT JOIN orders o
ON u.id = o.user_id;
```

---

## RIGHT JOIN

```sql
SELECT *
FROM users u
RIGHT JOIN orders o
ON u.id = o.user_id;
```

---

## CROSS JOIN

```sql
SELECT *
FROM table_a
CROSS JOIN table_b;
```

---

# 8. サブクエリ

## WHERE句

```sql
SELECT *
FROM users
WHERE id IN (
    SELECT user_id
    FROM orders
);
```

---

## FROM句

```sql
SELECT *
FROM (
    SELECT *
    FROM users
) t;
```

---

## EXISTS

```sql
SELECT *
FROM users u
WHERE EXISTS (
    SELECT 1
    FROM orders o
    WHERE o.user_id = u.id
);
```

---

# 9. UNION

## UNION

重複除外

```sql
SELECT name FROM users
UNION
SELECT name FROM employees;
```

---

## UNION ALL

重複保持

```sql
SELECT name FROM users
UNION ALL
SELECT name FROM employees;
```

---

# 10. CASE式

```sql
SELECT
    name,
    CASE
        WHEN age < 20 THEN 'Teen'
        WHEN age < 60 THEN 'Adult'
        ELSE 'Senior'
    END AS category
FROM users;
```

---

# 11. インデックス

## 作成

```sql
CREATE INDEX idx_users_email
ON users(email);
```

---

## 複合インデックス

```sql
CREATE INDEX idx_users_name_age
ON users(name, age);
```

---

## 削除

```sql
DROP INDEX idx_users_email
ON users;
```

---

# 12. トランザクション

## 開始

```sql
START TRANSACTION;
```

---

## コミット

```sql
COMMIT;
```

---

## ロールバック

```sql
ROLLBACK;
```

---

## 例

```sql
START TRANSACTION;

UPDATE accounts
SET balance = balance - 1000
WHERE id = 1;

UPDATE accounts
SET balance = balance + 1000
WHERE id = 2;

COMMIT;
```

---

# 13. ビュー

## 作成

```sql
CREATE VIEW active_users AS
SELECT *
FROM users
WHERE status = 'ACTIVE';
```

---

## 利用

```sql
SELECT *
FROM active_users;
```

---

# 14. ストアドプロシージャ

```sql
DELIMITER //

CREATE PROCEDURE GetUsers()
BEGIN
    SELECT *
    FROM users;
END //

DELIMITER ;
```

実行

```sql
CALL GetUsers();
```

---

# 15. MySQL 8.0の主要機能

## Common Table Expression (CTE)

```sql
WITH adults AS (
    SELECT *
    FROM users
    WHERE age >= 20
)
SELECT *
FROM adults;
```

---

## 再帰CTE

```sql
WITH RECURSIVE nums AS (
    SELECT 1 AS n
    UNION ALL
    SELECT n + 1
    FROM nums
    WHERE n < 10
)
SELECT *
FROM nums;
```

---

## ウィンドウ関数

```sql
SELECT
    name,
    salary,
    RANK() OVER (
        ORDER BY salary DESC
    ) AS ranking
FROM employees;
```

---

## ROW_NUMBER

```sql
SELECT
    ROW_NUMBER() OVER(
        ORDER BY id
    ) AS row_num,
    *
FROM users;
```

---

# 16. 実行順序

SQLは以下の順で処理される。

```text
FROM
JOIN
WHERE
GROUP BY
HAVING
SELECT
DISTINCT
ORDER BY
LIMIT
```

例

```sql
SELECT DISTINCT
    age
FROM users
WHERE age >= 20
GROUP BY age
HAVING COUNT(*) >= 2
ORDER BY age
LIMIT 10;
```

---

# 17. EXPLAIN

実行計画確認

```sql
EXPLAIN
SELECT *
FROM users
WHERE email = 'test@example.com';
```

---

## EXPLAIN ANALYZE

実測付き

```sql
EXPLAIN ANALYZE
SELECT *
FROM users
WHERE email = 'test@example.com';
```

---

# 18. パフォーマンスチューニング

## 避けるべき例

```sql
SELECT *
FROM users;
```

必要列だけ取得

```sql
SELECT
    id,
    name
FROM users;
```

---

## インデックス利用

```sql
CREATE INDEX idx_email
ON users(email);
```

---

## LIMIT利用

```sql
SELECT *
FROM users
ORDER BY id
LIMIT 100;
```

---

## OFFSET大量使用回避

悪い例

```sql
SELECT *
FROM users
ORDER BY id
LIMIT 100 OFFSET 100000;
```

良い例

```sql
SELECT *
FROM users
WHERE id > 100000
ORDER BY id
LIMIT 100;
```

---

# 19. よく使う管理コマンド

現在接続ユーザー

```sql
SELECT USER();
```

---

現在DB

```sql
SELECT DATABASE();
```

---

MySQLバージョン

```sql
SELECT VERSION();
```

---

接続確認

```sql
SHOW PROCESSLIST;
```

---

テーブルサイズ

```sql
SHOW TABLE STATUS;
```

---

インデックス確認

```sql
SHOW INDEX FROM users;
```

---

# 20. SQLコーディング規約例

推奨

```sql
SELECT
    u.id,
    u.name,
    o.amount
FROM users AS u
INNER JOIN orders AS o
    ON o.user_id = u.id
WHERE u.status = 'ACTIVE'
ORDER BY u.id;
```

推奨事項

- SQLキーワードは大文字
- テーブルにはエイリアスを付与
- SELECT * を避ける
- LIMITを付ける
- インデックスを意識したWHERE句を書く
- JOIN条件を明示する
- トランザクションを適切に利用する
- EXPLAINで実行計画を確認する

---
