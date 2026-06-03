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
