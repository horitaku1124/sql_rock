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
