use crate::error::{Result, SqlRockError};
use crate::model::{Column, SetClause, Statement, Table, WhereClause};
use crate::storage::{parse_table_file, serialize_table};
use std::fs;
use std::path::PathBuf;

pub struct Database {
    root: PathBuf,
}

impl Database {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn execute(&self, statement: Statement) -> Result<String> {
        match statement {
            Statement::CreateTable { name, columns } => self.create_table(&name, columns),
            Statement::InsertInto {
                table,
                columns,
                values,
            } => self.insert_into(&table, columns, values),
            Statement::SelectAll {
                table,
                where_clause,
            } => self.select_all(&table, where_clause),
            Statement::DescribeTable { table } => self.describe_table(&table),
            Statement::DropTable { table } => self.drop_table(&table),
            Statement::DeleteFrom {
                table,
                where_clause,
            } => self.delete_from(&table, where_clause),
            Statement::Update {
                table,
                set_clause,
                where_clause,
            } => self.update_rows(&table, set_clause, where_clause),
        }
    }

    fn create_table(&self, name: &str, columns: Vec<Column>) -> Result<String> {
        fs::create_dir_all(&self.root)?;

        let path = self.table_path(name)?;
        if path.exists() {
            return Err(SqlRockError::new(format!("table `{name}` already exists")));
        }

        let table = Table {
            name: name.to_string(),
            columns,
            rows: Vec::new(),
        };
        fs::write(path, serialize_table(&table))?;

        Ok(format!("created table `{name}`"))
    }

    fn insert_into(
        &self,
        table_name: &str,
        columns: Vec<String>,
        values: Vec<String>,
    ) -> Result<String> {
        if columns.len() != values.len() {
            return Err(SqlRockError::new(format!(
                "column count ({}) does not match value count ({})",
                columns.len(),
                values.len()
            )));
        }

        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let mut row = vec![String::new(); table.columns.len()];

        for (column_name, value) in columns.iter().zip(values.into_iter()) {
            let index = column_index(&table, column_name, table_name)?;
            row[index] = value;
        }

        table.rows.push(row);
        fs::write(path, serialize_table(&table))?;

        Ok(format!("inserted 1 row into `{table_name}`"))
    }

    fn select_all(&self, table_name: &str, where_clause: Option<WhereClause>) -> Result<String> {
        let table = self.read_table(table_name)?;
        let where_index = match &where_clause {
            Some(where_clause) => Some(column_index(&table, &where_clause.column, table_name)?),
            None => None,
        };
        let mut lines = Vec::new();
        lines.push(
            table
                .columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>()
                .join("\t"),
        );
        lines.extend(table.rows.iter().filter_map(|row| {
            let matches = match (&where_clause, where_index) {
                (Some(where_clause), Some(index)) => row.get(index) == Some(&where_clause.value),
                _ => true,
            };

            matches.then(|| row.join("\t"))
        }));

        Ok(lines.join("\n"))
    }

    fn describe_table(&self, table_name: &str) -> Result<String> {
        let table = self.read_table(table_name)?;
        let mut lines = Vec::new();
        lines.push("Field\tType".to_string());
        lines.extend(
            table
                .columns
                .iter()
                .map(|column| format!("{}\t{}", column.name, column.data_type)),
        );

        Ok(lines.join("\n"))
    }

    fn drop_table(&self, table_name: &str) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        fs::remove_file(path)?;
        Ok(format!("dropped table `{table_name}`"))
    }

    fn delete_from(&self, table_name: &str, where_clause: WhereClause) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let column_index = column_index(&table, &where_clause.column, table_name)?;

        let original_len = table.rows.len();
        table
            .rows
            .retain(|row| row.get(column_index) != Some(&where_clause.value));
        let deleted_count = original_len - table.rows.len();

        fs::write(path, serialize_table(&table))?;
        Ok(format!(
            "deleted {deleted_count} row(s) from `{table_name}`"
        ))
    }

    fn update_rows(
        &self,
        table_name: &str,
        set_clause: SetClause,
        where_clause: WhereClause,
    ) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let set_index = column_index(&table, &set_clause.column, table_name)?;
        let where_index = column_index(&table, &where_clause.column, table_name)?;

        let mut updated_count = 0;
        for row in &mut table.rows {
            if row.get(where_index) == Some(&where_clause.value) {
                row[set_index] = set_clause.value.clone();
                updated_count += 1;
            }
        }

        fs::write(path, serialize_table(&table))?;
        Ok(format!("updated {updated_count} row(s) in `{table_name}`"))
    }

    fn read_table(&self, table_name: &str) -> Result<Table> {
        let path = self.existing_table_path(table_name)?;
        parse_table_file(&fs::read_to_string(path)?)
    }

    fn existing_table_path(&self, table_name: &str) -> Result<PathBuf> {
        let path = self.table_path(table_name)?;
        if !path.exists() {
            return Err(SqlRockError::new(format!(
                "table `{table_name}` does not exist"
            )));
        }

        Ok(path)
    }

    fn table_path(&self, table_name: &str) -> Result<PathBuf> {
        validate_identifier(table_name)?;
        Ok(self.root.join(format!("{table_name}.table")))
    }
}

fn column_index(table: &Table, column_name: &str, table_name: &str) -> Result<usize> {
    table
        .columns
        .iter()
        .position(|column| column.name.eq_ignore_ascii_case(column_name))
        .ok_or_else(|| {
            SqlRockError::new(format!(
                "unknown column `{column_name}` for table `{table_name}`"
            ))
        })
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return Err(SqlRockError::new("identifier cannot be empty"));
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(SqlRockError::new(format!(
            "invalid identifier `{identifier}`"
        )));
    }

    if chars.any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_')) {
        return Err(SqlRockError::new(format!(
            "invalid identifier `{identifier}`"
        )));
    }

    Ok(())
}
