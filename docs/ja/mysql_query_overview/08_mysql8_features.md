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
