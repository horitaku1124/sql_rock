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