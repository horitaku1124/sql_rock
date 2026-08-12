use crate::error::{Result, SqlRockError};
use crate::model::{ForeignKey, ReferentialAction, Table};
use crate::storage::parse_table_file;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpOptions {
    pub tables: Vec<String>,
    pub include_create: bool,
    pub include_data: bool,
    pub add_drop_table: bool,
}

impl Default for DumpOptions {
    fn default() -> Self {
        Self {
            tables: Vec::new(),
            include_create: true,
            include_data: true,
            add_drop_table: false,
        }
    }
}

pub fn dump_directory(data_dir: &Path, options: &DumpOptions) -> Result<String> {
    let table_paths = selected_table_paths(data_dir, &options.tables)?;
    let mut tables = table_paths
        .into_iter()
        .map(|path| parse_table_file(&fs::read_to_string(path)?))
        .collect::<Result<Vec<_>>>()?;
    sort_tables_by_foreign_key_dependencies(&mut tables);
    let mut statements = Vec::new();

    for table in tables {
        append_table_dump(&mut statements, &table, options);
    }

    if statements.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{}\n", statements.join("\n\n")))
    }
}

fn sort_tables_by_foreign_key_dependencies(tables: &mut Vec<Table>) {
    let mut remaining = std::mem::take(tables);
    let mut sorted = Vec::new();
    while !remaining.is_empty() {
        let emitted_names = sorted
            .iter()
            .map(|table: &Table| table.name.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let remaining_names = remaining
            .iter()
            .map(|table| table.name.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let next = remaining.iter().position(|table| {
            table.foreign_keys.iter().all(|foreign_key| {
                let referenced = foreign_key.referenced_table.to_ascii_lowercase();
                emitted_names.contains(&referenced) || !remaining_names.contains(&referenced)
            })
        });
        let index = next.unwrap_or(0);
        sorted.push(remaining.remove(index));
    }
    *tables = sorted;
}

fn selected_table_paths(data_dir: &Path, tables: &[String]) -> Result<Vec<PathBuf>> {
    if !data_dir.is_dir() {
        return Err(SqlRockError::new(format!(
            "data directory `{}` does not exist",
            data_dir.display()
        )));
    }

    if !tables.is_empty() {
        let mut paths = Vec::with_capacity(tables.len());
        for table in tables {
            validate_table_name(table)?;
            let path = data_dir.join(format!("{table}.table"));
            if !path.is_file() {
                return Err(SqlRockError::new(format!("table `{table}` does not exist")));
            }
            paths.push(path);
        }
        return Ok(paths);
    }

    let mut paths = fs::read_dir(data_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "table")
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn append_table_dump(statements: &mut Vec<String>, table: &Table, options: &DumpOptions) {
    let table_name = quote_identifier(&table.name);
    if options.add_drop_table {
        statements.push(format!("DROP TABLE {table_name};"));
    }
    if options.include_create {
        statements.push(create_table_statement(table));
    }
    if options.include_data && !table.rows.is_empty() {
        statements.push(insert_statement(table));
    }
}

fn create_table_statement(table: &Table) -> String {
    let columns = table
        .columns
        .iter()
        .map(|column| format!("{} {}", quote_identifier(&column.name), column.data_type))
        .chain(table.foreign_keys.iter().map(format_foreign_key))
        .collect::<Vec<_>>()
        .join(",\n  ");
    let mut sql = format!(
        "CREATE TABLE {} (\n  {columns}\n)",
        quote_identifier(&table.name)
    );

    for option in &table.options {
        sql.push(' ');
        sql.push_str(&option.name);
        sql.push('=');
        sql.push_str(&option.value);
    }
    if let Some(value) = table.auto_increment_next {
        sql.push_str(&format!(" AUTO_INCREMENT={value}"));
    }
    if let Some(comment) = &table.comment {
        sql.push_str(" COMMENT=");
        sql.push_str(&quote_string(comment));
    }
    sql.push(';');
    sql
}

fn insert_statement(table: &Table) -> String {
    let columns = table
        .columns
        .iter()
        .map(|column| quote_identifier(&column.name))
        .collect::<Vec<_>>()
        .join(", ");
    let rows = table
        .rows
        .iter()
        .map(|row| {
            format!(
                "({})",
                row.iter()
                    .map(|value| quote_string(value))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(",\n  ");

    format!(
        "INSERT INTO {} ({columns}) VALUES\n  {rows};",
        quote_identifier(&table.name)
    )
}

fn quote_identifier(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn quote_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn format_foreign_key(foreign_key: &ForeignKey) -> String {
    let name = foreign_key
        .name
        .as_deref()
        .map(|name| format!("CONSTRAINT {} ", quote_identifier(name)))
        .unwrap_or_default();
    let columns = foreign_key
        .columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let referenced_columns = foreign_key
        .referenced_columns
        .iter()
        .map(|column| quote_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!(
        "{name}FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
        quote_identifier(&foreign_key.referenced_table)
    );
    if foreign_key.on_delete != ReferentialAction::Restrict {
        sql.push_str(" ON DELETE ");
        sql.push_str(referential_action_sql(foreign_key.on_delete));
    }
    if foreign_key.on_update != ReferentialAction::Restrict {
        sql.push_str(" ON UPDATE ");
        sql.push_str(referential_action_sql(foreign_key.on_update));
    }
    sql
}

fn referential_action_sql(action: ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Restrict => "RESTRICT",
        ReferentialAction::Cascade => "CASCADE",
        ReferentialAction::SetNull => "SET NULL",
        ReferentialAction::NoAction => "NO ACTION",
    }
}

fn validate_table_name(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(SqlRockError::new(format!("invalid table name `{value}`")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DumpOptions, dump_directory};
    use crate::database::Database;
    use crate::parser::{parse_statement, split_statements};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sql_rock_dump_{name}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn execute(database: &Database, sql: &str) {
        database.execute(parse_statement(sql).unwrap()).unwrap();
    }

    #[test]
    fn dumps_schema_and_rows_as_restorable_sql() {
        let source_dir = test_dir("source");
        let restore_dir = test_dir("restore");
        let source = Database::new(&source_dir);
        execute(
            &source,
            "CREATE TABLE users (id INT PRIMARY KEY AUTO_INCREMENT, name TEXT) ENGINE=INNODB AUTO_INCREMENT=10 COMMENT='user''s table';",
        );
        execute(
            &source,
            "INSERT INTO users (name) VALUES ('Alice'), ('O''Brien');",
        );

        let sql = dump_directory(&source_dir, &DumpOptions::default()).unwrap();
        assert!(sql.contains("CREATE TABLE `users`"));
        assert!(sql.contains("AUTO_INCREMENT=12"));
        assert!(sql.contains("'O''Brien'"));

        let restored = Database::new(&restore_dir);
        for statement in split_statements(&sql) {
            execute(&restored, &statement);
        }
        assert_eq!(
            restored
                .execute(parse_statement("SELECT * FROM users ORDER BY id;").unwrap())
                .unwrap(),
            "id\tname\n10\tAlice\n11\tO'Brien"
        );

        fs::remove_dir_all(source_dir).unwrap();
        fs::remove_dir_all(restore_dir).unwrap();
    }

    #[test]
    fn supports_table_selection_and_dump_sections() {
        let data_dir = test_dir("selection");
        let database = Database::new(&data_dir);
        execute(&database, "CREATE TABLE alpha (id INT);");
        execute(&database, "CREATE TABLE beta (id INT);");
        execute(&database, "INSERT INTO beta (id) VALUES (1);");

        let sql = dump_directory(
            &data_dir,
            &DumpOptions {
                tables: vec!["beta".to_string()],
                include_create: false,
                include_data: true,
                add_drop_table: true,
            },
        )
        .unwrap();
        assert_eq!(
            sql,
            "DROP TABLE `beta`;\n\nINSERT INTO `beta` (`id`) VALUES\n  ('1');\n"
        );

        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn dumps_foreign_key_tables_in_restorable_order() {
        let source_dir = test_dir("foreign_key_source");
        let restore_dir = test_dir("foreign_key_restore");
        let source = Database::new(&source_dir);
        execute(&source, "CREATE TABLE users (id INT PRIMARY KEY);");
        execute(
            &source,
            "CREATE TABLE orders (id INT PRIMARY KEY, user_id INT, FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE);",
        );
        execute(&source, "INSERT INTO users (id) VALUES (1);");
        execute(&source, "INSERT INTO orders (id, user_id) VALUES (10, 1);");

        let sql = dump_directory(&source_dir, &DumpOptions::default()).unwrap();
        assert!(sql.find("CREATE TABLE `users`") < sql.find("CREATE TABLE `orders`"));
        assert!(
            sql.contains("FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE")
        );

        let restored = Database::new(&restore_dir);
        for statement in split_statements(&sql) {
            execute(&restored, &statement);
        }
        assert_eq!(
            restored
                .execute(parse_statement("SELECT * FROM orders;").unwrap())
                .unwrap(),
            "id\tuser_id\n10\t1"
        );

        fs::remove_dir_all(source_dir).unwrap();
        fs::remove_dir_all(restore_dir).unwrap();
    }
}
