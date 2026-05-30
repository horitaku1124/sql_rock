pub mod cli;
pub mod database;
pub mod error;
pub mod model;
pub mod parser;
pub mod storage;

#[cfg(test)]
mod tests {
    use crate::database::Database;
    use crate::model::{Column, SetClause, Statement, WhereClause};
    use crate::parser::{parse_statement, split_statements};
    use crate::storage::parse_table_file;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "sql_rock_test_{}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            counter
        ))
    }

    fn setup_users_table() -> (PathBuf, Database) {
        let root = test_dir();
        let database = Database::new(&root);

        database
            .execute(parse_statement("CREATE TABLE users (id INT, name TEXT);").unwrap())
            .unwrap();
        database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (1, 'Alice');").unwrap())
            .unwrap();
        database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (2, 'Bob');").unwrap())
            .unwrap();

        (root, database)
    }

    #[test]
    fn parses_create_table() {
        let statement = parse_statement("CREATE TABLE users (id INT, name VARCHAR(255));").unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        data_type: "INT".to_string(),
                    },
                    Column {
                        name: "name".to_string(),
                        data_type: "VARCHAR(255)".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_insert_into() {
        let statement =
            parse_statement("INSERT INTO users (id, name) VALUES (1, 'Alice''s note');").unwrap();

        assert_eq!(
            statement,
            Statement::InsertInto {
                table: "users".to_string(),
                columns: vec!["id".to_string(), "name".to_string()],
                values: vec!["1".to_string(), "Alice's note".to_string()],
            }
        );
    }

    #[test]
    fn parses_select_all() {
        let statement = parse_statement("SELECT * FROM users;").unwrap();

        assert_eq!(
            statement,
            Statement::SelectAll {
                table: "users".to_string(),
                where_clause: None,
            }
        );
    }

    #[test]
    fn parses_select_with_where() {
        let statement = parse_statement("SELECT * FROM users WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::SelectAll {
                table: "users".to_string(),
                where_clause: Some(WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                }),
            }
        );
    }

    #[test]
    fn parses_desc() {
        let statement = parse_statement("DESC users;").unwrap();

        assert_eq!(
            statement,
            Statement::DescribeTable {
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn parses_drop_table() {
        let statement = parse_statement("DROP TABLE users;").unwrap();

        assert_eq!(
            statement,
            Statement::DropTable {
                table: "users".to_string(),
            }
        );
    }

    #[test]
    fn parses_delete_from() {
        let statement = parse_statement("DELETE FROM users WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::DeleteFrom {
                table: "users".to_string(),
                where_clause: WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                },
            }
        );
    }

    #[test]
    fn parses_update() {
        let statement = parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap();

        assert_eq!(
            statement,
            Statement::Update {
                table: "users".to_string(),
                set_clause: SetClause {
                    column: "name".to_string(),
                    value: "abc".to_string(),
                },
                where_clause: WhereClause {
                    column: "id".to_string(),
                    value: "1".to_string(),
                },
            }
        );
    }

    #[test]
    fn splits_multiple_statements_with_strings() {
        let statements = split_statements(
            "INSERT INTO users (id, name) VALUES (1, 'Alice; note'); SELECT * FROM users;",
        );

        assert_eq!(
            statements,
            vec![
                "INSERT INTO users (id, name) VALUES (1, 'Alice; note')".to_string(),
                "SELECT * FROM users".to_string(),
            ]
        );
    }

    #[test]
    fn create_table_insert_row_and_select() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();
        let filtered_output = database
            .execute(parse_statement("SELECT * FROM users WHERE id = 1;").unwrap())
            .unwrap();
        let filtered_name_output = database
            .execute(parse_statement("SELECT * FROM users WHERE name = 'Bob';").unwrap())
            .unwrap();

        let table =
            parse_table_file(&fs::read_to_string(root.join("users.table")).unwrap()).unwrap();
        assert_eq!(
            table.rows,
            vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()]
            ]
        );
        assert_eq!(output, "id\tname\n1\tAlice\n2\tBob");
        assert_eq!(filtered_output, "id\tname\n1\tAlice");
        assert_eq!(filtered_name_output, "id\tname\n2\tBob");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn describe_table_returns_schema() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DESC users;").unwrap())
            .unwrap();

        assert_eq!(output, "Field\tType\nid\tINT\nname\tTEXT");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn drop_table_removes_table_file() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DROP TABLE users;").unwrap())
            .unwrap();
        let select_error = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap_err();

        assert_eq!(output, "dropped table `users`");
        assert_eq!(select_error.to_string(), "table `users` does not exist");
        assert!(!root.join("users.table").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_from_removes_matching_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DELETE FROM users WHERE id = 1;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "deleted 1 row(s) from `users`");
        assert_eq!(select_output, "id\tname\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn delete_from_without_match_keeps_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("DELETE FROM users WHERE id = 999;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "deleted 0 row(s) from `users`");
        assert_eq!(select_output, "id\tname\n1\tAlice\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_rows_updates_matching_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "updated 1 row(s) in `users`");
        assert_eq!(select_output, "id\tname\n1\tabc\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_rows_without_match_keeps_rows() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 999;").unwrap())
            .unwrap();
        let select_output = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap();

        assert_eq!(output, "updated 0 row(s) in `users`");
        assert_eq!(select_output, "id\tname\n1\tAlice\n2\tBob");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_where_with_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("SELECT * FROM users WHERE age = 20;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn insert_into_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("INSERT INTO users (age) VALUES (20);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_from_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("SELECT * FROM users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn desc_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DESC users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn drop_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DROP TABLE users;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn delete_from_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("DELETE FROM users WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn delete_from_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("DELETE FROM users WHERE age = 20;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_missing_table_returns_error() {
        let root = test_dir();
        let database = Database::new(&root);

        let error = database
            .execute(parse_statement("UPDATE users SET name = 'abc' WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` does not exist");
    }

    #[test]
    fn update_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("UPDATE users SET age = 20 WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "unknown column `age` for table `users`");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_existing_table_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("CREATE TABLE users (id INT, name TEXT);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "table `users` already exists");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn select_where_without_match_returns_header_only() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("SELECT * FROM users WHERE id = 999;").unwrap())
            .unwrap();

        assert_eq!(output, "id\tname");
        fs::remove_dir_all(root).unwrap();
    }
}
