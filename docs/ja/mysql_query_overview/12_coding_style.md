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
