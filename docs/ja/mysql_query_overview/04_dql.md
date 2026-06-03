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
