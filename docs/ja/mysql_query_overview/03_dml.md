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
