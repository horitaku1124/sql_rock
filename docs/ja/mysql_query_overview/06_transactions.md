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
