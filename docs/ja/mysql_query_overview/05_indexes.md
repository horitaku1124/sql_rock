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
