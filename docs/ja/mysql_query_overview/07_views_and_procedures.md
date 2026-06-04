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
