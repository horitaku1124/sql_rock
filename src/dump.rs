use crate::error::{Result, SqlRockError};
use crate::model::Table;
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
    let mut statements = Vec::new();

    for path in table_paths {
        let table = parse_table_file(&fs::read_to_string(path)?)?;
        append_table_dump(&mut statements, &table, options);
    }

    if statements.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{}\n", statements.join("\n\n")))
    }
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
}
