pub mod cli;
pub mod data_type;
pub mod database;
pub mod error;
pub mod model;
pub mod parser;
pub mod query_engine;
pub mod query_parser;
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

    fn execute_sql(database: &Database, sql: &str) -> String {
        database.execute(parse_statement(sql).unwrap()).unwrap()
    }

    #[test]
    fn parses_create_table() {
        let statement = parse_statement("CREATE TABLE users (id INT, name VARCHAR(255));").unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                comment: None,
                auto_increment_start: None,
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
    fn parses_create_table_with_primary_key_and_unique_key() {
        let statement = parse_statement(
            "CREATE TABLE users (id INT, email TEXT, PRIMARY KEY (id), UNIQUE KEY unique_email (email));",
        )
        .unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                comment: None,
                auto_increment_start: None,
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        data_type: "INT PRIMARY KEY".to_string(),
                    },
                    Column {
                        name: "email".to_string(),
                        data_type: "TEXT UNIQUE KEY".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_create_table_with_table_comment() {
        let statement =
            parse_statement("CREATE TABLE users (id INT, name TEXT) COMMENT='ユーザー情報''管理';")
                .unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                comment: Some("ユーザー情報'管理".to_string()),
                auto_increment_start: None,
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        data_type: "INT".to_string(),
                    },
                    Column {
                        name: "name".to_string(),
                        data_type: "TEXT".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_create_table_with_auto_increment_start() {
        let statement = parse_statement(
            "CREATE TABLE users (id INT AUTO_INCREMENT, name TEXT) AUTO_INCREMENT=100 COMMENT='users';",
        )
        .unwrap();

        assert_eq!(
            statement,
            Statement::CreateTable {
                name: "users".to_string(),
                comment: Some("users".to_string()),
                auto_increment_start: Some(100),
                columns: vec![
                    Column {
                        name: "id".to_string(),
                        data_type: "INT AUTO_INCREMENT".to_string(),
                    },
                    Column {
                        name: "name".to_string(),
                        data_type: "TEXT".to_string(),
                    },
                ],
            }
        );
    }

    #[test]
    fn parses_create_table_with_supported_data_types() {
        let data_types = [
            "TINYINT",
            "SMALLINT",
            "MEDIUMINT",
            "INT",
            "INTEGER",
            "BIGINT",
            "DECIMAL(10, 2)",
            "NUMERIC",
            "DEC",
            "FIXED",
            "FLOAT",
            "DOUBLE PRECISION",
            "REAL",
            "BIT(8)",
            "DATE",
            "TIME(6)",
            "DATETIME",
            "TIMESTAMP",
            "YEAR",
            "CHAR(10)",
            "VARCHAR(255)",
            "BINARY(16)",
            "VARBINARY(255)",
            "TINYBLOB",
            "BLOB",
            "MEDIUMBLOB",
            "LONGBLOB",
            "TINYTEXT",
            "TEXT",
            "MEDIUMTEXT",
            "LONGTEXT",
            "ENUM('a', 'b')",
            "SET('a', 'b')",
            "VECTOR(3)",
            "GEOMETRY",
            "POINT",
            "LINESTRING",
            "POLYGON",
            "MULTIPOINT",
            "MULTILINESTRING",
            "MULTIPOLYGON",
            "GEOMETRYCOLLECTION",
            "JSON",
        ];

        for data_type in data_types {
            let sql = format!("CREATE TABLE values_table (value {data_type});");
            assert!(
                parse_statement(&sql).is_ok(),
                "{data_type} should be supported"
            );
        }
    }

    #[test]
    fn rejects_create_table_with_unsupported_data_type() {
        let error = parse_statement("CREATE TABLE users (id INTT);").unwrap_err();

        assert_eq!(error.to_string(), "unsupported data type `INTT`");
    }

    #[test]
    fn rejects_alter_table_with_unsupported_data_type() {
        let error =
            parse_statement("ALTER TABLE users ADD COLUMN status UNKNOWN_TYPE;").unwrap_err();

        assert_eq!(error.to_string(), "unsupported data type `UNKNOWN_TYPE`");
    }

    #[test]
    fn auto_increment_generates_and_persists_sequence_values() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT AUTO_INCREMENT, name TEXT);",
        );

        execute_sql(&database, "INSERT INTO users (name) VALUES ('Taro');");
        execute_sql(
            &database,
            "INSERT INTO users (id, name) VALUES (NULL, 'Jiro'), (0, 'Saburo');",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, name) VALUES (10, 'Shiro');",
        );
        execute_sql(&database, "INSERT INTO users (name) VALUES ('Goro');");
        execute_sql(&database, "DELETE FROM users WHERE id = 11;");
        execute_sql(&database, "INSERT INTO users (name) VALUES ('Rokuro');");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n1\tTaro\n2\tJiro\n3\tSaburo\n10\tShiro\n12\tRokuro"
        );
        assert!(
            fs::read_to_string(root.join("users.table"))
                .unwrap()
                .contains("auto_increment:13\n")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn auto_increment_uses_create_table_start_value() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT AUTO_INCREMENT, name TEXT) AUTO_INCREMENT=100;",
        );

        execute_sql(&database, "INSERT INTO users (name) VALUES ('Taro');");
        execute_sql(&database, "INSERT INTO users (name) VALUES ('Jiro');");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n100\tTaro\n101\tJiro"
        );
        assert!(
            fs::read_to_string(root.join("users.table"))
                .unwrap()
                .contains("auto_increment:102\n")
        );
        assert_eq!(
            execute_sql(&database, "SHOW CREATE TABLE users;"),
            "Table\tCreate Table\nusers\tCREATE TABLE users (id INT AUTO_INCREMENT, name TEXT) AUTO_INCREMENT=102"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn auto_increment_advances_after_update_and_resets_after_truncate() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT AUTO_INCREMENT, name TEXT);",
        );

        execute_sql(&database, "INSERT INTO users (name) VALUES ('Taro');");
        execute_sql(&database, "UPDATE users SET id = 20 WHERE id = 1;");
        execute_sql(&database, "INSERT INTO users (name) VALUES ('Jiro');");
        execute_sql(&database, "TRUNCATE TABLE users;");
        execute_sql(&database, "INSERT INTO users (name) VALUES ('Saburo');");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n1\tSaburo"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_auto_increment_definitions() {
        let error = parse_statement("CREATE TABLE users (id TEXT AUTO_INCREMENT);").unwrap_err();
        assert_eq!(
            error.to_string(),
            "AUTO_INCREMENT column `id` requires an integer data type"
        );

        let error = parse_statement(
            "CREATE TABLE users (id INT AUTO_INCREMENT, serial BIGINT AUTO_INCREMENT);",
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "only one AUTO_INCREMENT column is allowed"
        );
    }

    #[test]
    fn rejects_invalid_key_definitions() {
        let error = parse_statement(
            "CREATE TABLE users (id INT PRIMARY KEY, code INT, PRIMARY KEY (code));",
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "only one PRIMARY KEY is allowed");

        let error =
            parse_statement("CREATE TABLE users (id INT, PRIMARY KEY (missing));").unwrap_err();
        assert_eq!(
            error.to_string(),
            "unknown column `missing` in key definition"
        );

        let error =
            parse_statement("CREATE TABLE users (id INT, code INT, PRIMARY KEY (id, code));")
                .unwrap_err();
        assert_eq!(error.to_string(), "PRIMARY KEY supports exactly one column");
    }

    #[test]
    fn primary_key_and_unique_key_reject_duplicate_inserts() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT PRIMARY KEY, email TEXT UNIQUE KEY, name TEXT);",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, email, name) VALUES (1, 'a@example.com', 'Alice');",
        );

        let error = database
            .execute(
                parse_statement(
                    "INSERT INTO users (id, email, name) VALUES (1, 'b@example.com', 'Bob');",
                )
                .unwrap(),
            )
            .unwrap_err();
        assert_eq!(error.to_string(), "duplicate value `1` for key `id`");

        let error = database
            .execute(
                parse_statement(
                    "INSERT INTO users (id, email, name) VALUES (2, 'a@example.com', 'Bob');",
                )
                .unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "duplicate value `a@example.com` for key `email`"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn primary_key_and_unique_key_reject_invalid_updates() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT PRIMARY KEY, email TEXT UNIQUE KEY, name TEXT);",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, email, name) VALUES (1, 'a@example.com', 'Alice'), (2, 'b@example.com', 'Bob');",
        );

        let error = database
            .execute(parse_statement("UPDATE users SET id = 1 WHERE id = 2;").unwrap())
            .unwrap_err();
        assert_eq!(error.to_string(), "duplicate value `1` for key `id`");

        let error = database
            .execute(
                parse_statement("UPDATE users SET email = 'a@example.com' WHERE id = 2;").unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "duplicate value `a@example.com` for key `email`"
        );

        let error = database
            .execute(parse_statement("UPDATE users SET id = NULL WHERE id = 2;").unwrap())
            .unwrap_err();
        assert_eq!(error.to_string(), "column `id` cannot be NULL");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users ORDER BY id;"),
            "id\temail\tname\n1\ta@example.com\tAlice\n2\tb@example.com\tBob"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_null_insert_into_not_null_column() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT NOT NULL);",
        );

        let error = database
            .execute(parse_statement("INSERT INTO users (id, name) VALUES (1, NULL);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "column `name` cannot be NULL");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_omitted_insert_into_not_null_column() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT NOT NULL);",
        );

        let error = database
            .execute(parse_statement("INSERT INTO users (id) VALUES (1);").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "column `name` cannot be NULL");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn allows_empty_string_insert_into_not_null_column() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT NOT NULL);",
        );

        execute_sql(&database, "INSERT INTO users (id, name) VALUES (1, '');");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n1\t"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_null_update_into_not_null_column() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT NOT NULL);",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, name) VALUES (1, 'Taro');",
        );

        let error = database
            .execute(parse_statement("UPDATE users SET name = NULL WHERE id = 1;").unwrap())
            .unwrap_err();

        assert_eq!(error.to_string(), "column `name` cannot be NULL");
        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n1\tTaro"
        );
        fs::remove_dir_all(root).unwrap();
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
    fn parses_select_count() {
        let statement = parse_statement("SELECT count(id) FROM users;").unwrap();

        assert_eq!(
            statement,
            Statement::SelectCount {
                table: "users".to_string(),
                column: "id".to_string(),
                where_clause: None,
            }
        );
    }

    #[test]
    fn parses_select_count_with_where() {
        let statement = parse_statement("SELECT count(id) FROM users WHERE name = 'Bob';").unwrap();

        assert_eq!(
            statement,
            Statement::SelectCount {
                table: "users".to_string(),
                column: "id".to_string(),
                where_clause: Some(WhereClause {
                    column: "name".to_string(),
                    value: "Bob".to_string(),
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
    fn parses_legacy_table_file_without_auto_increment_metadata() {
        let table =
            parse_table_file("table:users\ncolumns:id:INT|name:TEXT\nrows:\n1|Alice\n").unwrap();

        assert_eq!(table.comment, None);
        assert_eq!(table.auto_increment_next, None);
        assert_eq!(table.rows, vec![vec!["1".to_string(), "Alice".to_string()]]);
    }

    #[test]
    fn stores_and_reads_table_comment() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT) COMMENT='user''s table';",
        );

        let table =
            parse_table_file(&fs::read_to_string(root.join("users.table")).unwrap()).unwrap();

        assert_eq!(table.comment, Some("user's table".to_string()));
        assert!(
            fs::read_to_string(root.join("users.table"))
                .unwrap()
                .contains("comment:user's table\n")
        );
        assert_eq!(
            execute_sql(&database, "SHOW CREATE TABLE users;"),
            "Table\tCreate Table\nusers\tCREATE TABLE users (id INT, name TEXT) COMMENT='user''s table'"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn create_table_supports_comment_and_auto_increment_options_in_any_order() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT AUTO_INCREMENT, name TEXT) COMMENT='users' AUTO_INCREMENT=50;",
        );

        execute_sql(&database, "INSERT INTO users (name) VALUES ('Taro');");

        assert_eq!(
            execute_sql(&database, "SELECT * FROM users;"),
            "id\tname\n50\tTaro"
        );
        assert_eq!(
            execute_sql(&database, "SHOW CREATE TABLE users;"),
            "Table\tCreate Table\nusers\tCREATE TABLE users (id INT AUTO_INCREMENT, name TEXT) COMMENT='users' AUTO_INCREMENT=51"
        );
        fs::remove_dir_all(root).unwrap();
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
    fn select_count_returns_matching_count() {
        let (root, database) = setup_users_table();

        let output = database
            .execute(parse_statement("SELECT count(id) FROM users;").unwrap())
            .unwrap();
        let filtered_output = database
            .execute(parse_statement("SELECT count(id) FROM users WHERE name = 'Bob';").unwrap())
            .unwrap();

        assert_eq!(output, "count(id)\n2");
        assert_eq!(filtered_output, "count(id)\n1");
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
    fn select_count_unknown_column_returns_error() {
        let (root, database) = setup_users_table();

        let error = database
            .execute(parse_statement("SELECT count(age) FROM users;").unwrap())
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

    #[test]
    fn advanced_select_filters_sorts_limits_and_aggregates() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT, age INT, email TEXT);",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, name, age, email) VALUES (1, 'Taro', 20, 'taro@example.com'), (2, 'Jiro', 25, NULL), (3, 'Taro', 30, 'taro2@example.com');",
        );

        assert_eq!(
            execute_sql(
                &database,
                "SELECT id, name FROM users WHERE age >= 20 ORDER BY age DESC LIMIT 2;"
            ),
            "id\tname\n3\tTaro\n2\tJiro"
        );
        assert_eq!(
            execute_sql(&database, "SELECT DISTINCT name FROM users ORDER BY name;"),
            "name\nJiro\nTaro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT COUNT(*), SUM(age), AVG(age), MAX(age), MIN(age) FROM users;"
            ),
            "count(*)\tsum(age)\tavg(age)\tmax(age)\tmin(age)\n3\t75\t25\t30\t20"
        );
        assert_eq!(
            execute_sql(&database, "SELECT id FROM users WHERE email IS NULL;"),
            "id\n2"
        );
        assert_eq!(
            execute_sql(&database, "SELECT name FROM users WHERE name LIKE 'Ta%';"),
            "name\nTaro\nTaro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT id FROM users WHERE age >= 20 AND name = 'Taro' ORDER BY id;"
            ),
            "id\n1\n3"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT id FROM users WHERE age < 21 OR age > 29 ORDER BY id;"
            ),
            "id\n1\n3"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn advanced_select_groups_and_evaluates_case() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(
            &database,
            "CREATE TABLE users (id INT, name TEXT, age INT);",
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, name, age) VALUES (1, 'Taro', 20), (2, 'Jiro', 20), (3, 'Saburo', 30);",
        );

        assert_eq!(
            execute_sql(
                &database,
                "SELECT age, COUNT(*) FROM users GROUP BY age HAVING COUNT(*) >= 2 ORDER BY age;"
            ),
            "age\tcount(*)\n20\t2"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name, CASE WHEN age < 25 THEN 'Young' ELSE 'Adult' END AS category FROM users ORDER BY id;"
            ),
            "name\tcategory\nTaro\tYoung\nJiro\tYoung\nSaburo\tAdult"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn advanced_select_joins_subqueries_unions_and_insert_select() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(&database, "CREATE TABLE users (id INT, name TEXT);");
        execute_sql(
            &database,
            "CREATE TABLE orders (id INT, user_id INT, amount INT);",
        );
        execute_sql(&database, "CREATE TABLE archive_users (id INT, name TEXT);");
        execute_sql(
            &database,
            "INSERT INTO users (id, name) VALUES (1, 'Taro'), (2, 'Jiro');",
        );
        execute_sql(
            &database,
            "INSERT INTO orders (id, user_id, amount) VALUES (10, 1, 500), (11, 1, 700), (12, 9, 100);",
        );

        assert_eq!(
            execute_sql(
                &database,
                "SELECT u.name, o.amount FROM users u INNER JOIN orders o ON u.id = o.user_id ORDER BY o.amount;"
            ),
            "u.name\to.amount\nTaro\t500\nTaro\t700"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT u.name, o.amount FROM users u LEFT JOIN orders o ON u.id = o.user_id ORDER BY u.id, o.amount;"
            ),
            "u.name\to.amount\nTaro\t500\nTaro\t700\nJiro\t"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT u.name, o.amount FROM users u RIGHT JOIN orders o ON u.id = o.user_id ORDER BY o.id;"
            ),
            "u.name\to.amount\nTaro\t500\nTaro\t700\n\t100"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT u.name, o.amount FROM users u CROSS JOIN orders o ORDER BY u.id, o.id LIMIT 2;"
            ),
            "u.name\to.amount\nTaro\t500\nTaro\t700"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name FROM users WHERE id IN (SELECT user_id FROM orders) ORDER BY name;"
            ),
            "name\nTaro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name FROM users u WHERE EXISTS (SELECT id FROM orders o WHERE o.user_id = u.id) ORDER BY name;"
            ),
            "name\nTaro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name FROM (SELECT name FROM users) t ORDER BY name;"
            ),
            "name\nJiro\nTaro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name FROM users UNION SELECT name FROM users;"
            ),
            "name\nTaro\nJiro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "SELECT name FROM users UNION ALL SELECT name FROM users;"
            ),
            "name\nTaro\nJiro\nTaro\nJiro"
        );
        assert_eq!(
            execute_sql(
                &database,
                "INSERT INTO archive_users SELECT id, name FROM users WHERE id > 1;"
            ),
            "inserted 1 row(s) into `archive_users`"
        );
        assert_eq!(
            execute_sql(&database, "SELECT * FROM archive_users;"),
            "id\tname\n2\tJiro"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ddl_extensions_show_alter_delete_all_and_truncate() {
        let root = test_dir();
        let database = Database::new(&root);
        execute_sql(&database, "CREATE TABLE users (id INT, name TEXT);");
        execute_sql(
            &database,
            "INSERT INTO users (id, name) VALUES (1, 'Taro'), (2, 'Jiro');",
        );

        assert_eq!(execute_sql(&database, "SHOW TABLES;"), "Tables\nusers");
        assert_eq!(
            execute_sql(&database, "SHOW CREATE TABLE users;"),
            "Table\tCreate Table\nusers\tCREATE TABLE users (id INT, name TEXT)"
        );
        execute_sql(&database, "ALTER TABLE users ADD COLUMN age INT;");
        execute_sql(&database, "ALTER TABLE users MODIFY COLUMN age SMALLINT;");
        execute_sql(
            &database,
            "ALTER TABLE users CHANGE COLUMN name full_name VARCHAR(100);",
        );
        assert_eq!(
            execute_sql(&database, "DESCRIBE users;"),
            "Field\tType\nid\tINT\nfull_name\tVARCHAR(100)\nage\tSMALLINT"
        );
        assert_eq!(
            execute_sql(&database, "DELETE FROM users;"),
            "deleted 2 row(s) from `users`"
        );
        execute_sql(
            &database,
            "INSERT INTO users (id, full_name) VALUES (3, 'Saburo');",
        );
        assert_eq!(
            execute_sql(&database, "TRUNCATE TABLE users;"),
            "deleted 1 row(s) from `users`"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
