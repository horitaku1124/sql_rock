use crate::data_type::{has_auto_increment, has_not_null, validate_auto_increment_columns};
use crate::error::{Result, SqlRockError};
use crate::model::{AlterTableAction, Column, SQL_NULL, SetClause, Statement, Table, WhereClause};
use crate::query_engine::execute_query;
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
            Statement::InsertRows {
                table,
                columns,
                rows,
            } => self.insert_rows(&table, columns, rows),
            Statement::InsertSelect { table, query } => self.insert_select(&table, &query),
            Statement::SelectAll {
                table,
                where_clause,
            } => self.select_all(&table, where_clause),
            Statement::SelectCount {
                table,
                column,
                where_clause,
            } => self.select_count(&table, &column, where_clause),
            Statement::DescribeTable { table } => self.describe_table(&table),
            Statement::ShowTables => self.show_tables(),
            Statement::ShowCreateTable { table } => self.show_create_table(&table),
            Statement::AlterTable { table, action } => self.alter_table(&table, action),
            Statement::DropTable { table } => self.drop_table(&table),
            Statement::DeleteFrom {
                table,
                where_clause,
            } => self.delete_from(&table, where_clause),
            Statement::DeleteAll { table } => self.delete_all(&table),
            Statement::TruncateTable { table } => self.truncate_table(&table),
            Statement::Update {
                table,
                set_clause,
                where_clause,
            } => self.update_rows(&table, set_clause, where_clause),
            Statement::UpdateMany {
                table,
                set_clauses,
                where_clause,
            } => self.update_many(&table, set_clauses, where_clause),
            Statement::SelectQuery(query) => {
                Ok(execute_query(&query, &|table| self.read_table(table))?.render())
            }
        }
    }

    fn create_table(&self, name: &str, columns: Vec<Column>) -> Result<String> {
        fs::create_dir_all(&self.root)?;
        validate_auto_increment_columns(&columns)?;

        let path = self.table_path(name)?;
        if path.exists() {
            return Err(SqlRockError::new(format!("table `{name}` already exists")));
        }

        let mut table = Table {
            name: name.to_string(),
            columns,
            auto_increment_next: None,
            rows: Vec::new(),
        };
        sync_auto_increment_next(&mut table)?;
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
        let mut provided = vec![false; table.columns.len()];

        for (column_name, value) in columns.iter().zip(values.into_iter()) {
            let index = column_index(&table, column_name, table_name)?;
            row[index] = value;
            provided[index] = true;
        }

        apply_auto_increment(&mut table, &mut row)?;
        validate_not_null_values(&table, &row, &provided)?;
        normalize_null_values(&mut row);
        table.rows.push(row);
        fs::write(path, serialize_table(&table))?;

        Ok(format!("inserted 1 row into `{table_name}`"))
    }

    fn insert_rows(
        &self,
        table_name: &str,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    ) -> Result<String> {
        let count = rows.len();
        for values in rows {
            self.insert_into(table_name, columns.clone(), values)?;
        }
        Ok(format!("inserted {count} row(s) into `{table_name}`"))
    }

    fn insert_select(&self, table_name: &str, query: &crate::model::SelectQuery) -> Result<String> {
        let target = self.read_table(table_name)?;
        let result = execute_query(query, &|table| self.read_table(table))?;
        if result.headers.len() != target.columns.len() {
            return Err(SqlRockError::new(
                "INSERT SELECT column count does not match target table",
            ));
        }
        self.insert_rows(
            table_name,
            target
                .columns
                .into_iter()
                .map(|column| column.name)
                .collect(),
            result.rows,
        )
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

    fn show_tables(&self) -> Result<String> {
        fs::create_dir_all(&self.root)?;
        let mut tables = fs::read_dir(&self.root)?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                entry
                    .path()
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();
        tables.sort();
        let mut lines = vec!["Tables".to_string()];
        lines.extend(tables);
        Ok(lines.join("\n"))
    }

    fn show_create_table(&self, table_name: &str) -> Result<String> {
        let table = self.read_table(table_name)?;
        let columns = table
            .columns
            .iter()
            .map(|column| format!("{} {}", column.name, column.data_type))
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!(
            "Table\tCreate Table\n{table_name}\tCREATE TABLE {table_name} ({columns})"
        ))
    }

    fn alter_table(&self, table_name: &str, action: AlterTableAction) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        match action {
            AlterTableAction::Add(column) => {
                ensure_column_missing(&table, &column.name, table_name)?;
                table.columns.push(column);
                for row in &mut table.rows {
                    row.push(String::new());
                }
            }
            AlterTableAction::Modify(column) => {
                let index = column_index(&table, &column.name, table_name)?;
                table.columns[index].data_type = column.data_type;
            }
            AlterTableAction::Change { old_name, column } => {
                let index = column_index(&table, &old_name, table_name)?;
                if !old_name.eq_ignore_ascii_case(&column.name) {
                    ensure_column_missing(&table, &column.name, table_name)?;
                }
                table.columns[index] = column;
            }
        }
        validate_auto_increment_columns(&table.columns)?;
        sync_auto_increment_next(&mut table)?;
        fs::write(path, serialize_table(&table))?;
        Ok(format!("altered table `{table_name}`"))
    }

    fn select_count(
        &self,
        table_name: &str,
        count_column: &str,
        where_clause: Option<WhereClause>,
    ) -> Result<String> {
        let table = self.read_table(table_name)?;
        let count_index = column_index(&table, count_column, table_name)?;
        let where_index = match &where_clause {
            Some(where_clause) => Some(column_index(&table, &where_clause.column, table_name)?),
            None => None,
        };

        let count = table
            .rows
            .iter()
            .filter(|row| match (&where_clause, where_index) {
                (Some(where_clause), Some(index)) => row.get(index) == Some(&where_clause.value),
                _ => true,
            })
            .filter(|row| row.get(count_index).is_some_and(|value| !value.is_empty()))
            .count();

        Ok(format!("count({count_column})\n{count}"))
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

    fn delete_all(&self, table_name: &str) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let deleted_count = table.rows.len();
        table.rows.clear();
        fs::write(path, serialize_table(&table))?;
        Ok(format!(
            "deleted {deleted_count} row(s) from `{table_name}`"
        ))
    }

    fn truncate_table(&self, table_name: &str) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let deleted_count = table.rows.len();
        table.rows.clear();
        table.auto_increment_next = auto_increment_index(&table).map(|_| 1);
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
                let value = normalize_updated_value(&table.columns[set_index], &set_clause.value)?;
                row[set_index] = value;
                updated_count += 1;
            }
        }

        sync_auto_increment_next(&mut table)?;
        fs::write(path, serialize_table(&table))?;
        Ok(format!("updated {updated_count} row(s) in `{table_name}`"))
    }

    fn update_many(
        &self,
        table_name: &str,
        set_clauses: Vec<SetClause>,
        where_clause: WhereClause,
    ) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let updates = set_clauses
            .iter()
            .map(|set| {
                let index = column_index(&table, &set.column, table_name)?;
                Ok((
                    index,
                    normalize_updated_value(&table.columns[index], &set.value)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let where_index = column_index(&table, &where_clause.column, table_name)?;
        let mut updated_count = 0;
        for row in &mut table.rows {
            if row.get(where_index) == Some(&where_clause.value) {
                for (index, value) in &updates {
                    row[*index] = value.clone();
                }
                updated_count += 1;
            }
        }
        sync_auto_increment_next(&mut table)?;
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

fn apply_auto_increment(table: &mut Table, row: &mut [String]) -> Result<()> {
    let Some(index) = auto_increment_index(table) else {
        return Ok(());
    };

    if row[index].is_empty() || row[index] == "0" || row[index] == SQL_NULL {
        let value = table.auto_increment_next.unwrap_or(1);
        row[index] = value.to_string();
        table.auto_increment_next = Some(increment_auto_value(value)?);
    } else {
        let value = parse_auto_increment_value(&row[index])?;
        let next = increment_auto_value(value)?;
        table.auto_increment_next = Some(table.auto_increment_next.unwrap_or(1).max(next));
    }

    Ok(())
}

fn validate_not_null_values(table: &Table, row: &[String], provided: &[bool]) -> Result<()> {
    for (index, column) in table.columns.iter().enumerate() {
        if has_not_null(&column.data_type)
            && (!provided[index] || row.get(index).is_some_and(|value| value == SQL_NULL))
        {
            return Err(SqlRockError::new(format!(
                "column `{}` cannot be NULL",
                column.name
            )));
        }
    }
    Ok(())
}

fn normalize_null_values(row: &mut [String]) {
    for value in row {
        if value == SQL_NULL {
            value.clear();
        }
    }
}

fn normalize_updated_value(column: &Column, value: &str) -> Result<String> {
    if value == SQL_NULL {
        if has_not_null(&column.data_type) {
            return Err(SqlRockError::new(format!(
                "column `{}` cannot be NULL",
                column.name
            )));
        }
        return Ok(String::new());
    }
    Ok(value.to_string())
}

fn sync_auto_increment_next(table: &mut Table) -> Result<()> {
    let Some(index) = auto_increment_index(table) else {
        table.auto_increment_next = None;
        return Ok(());
    };

    let mut next = table.auto_increment_next.unwrap_or(1);
    for row in &table.rows {
        if let Some(value) = row.get(index).filter(|value| !value.is_empty()) {
            next = next.max(increment_auto_value(parse_auto_increment_value(value)?)?);
        }
    }
    table.auto_increment_next = Some(next);
    Ok(())
}

fn auto_increment_index(table: &Table) -> Option<usize> {
    table
        .columns
        .iter()
        .position(|column| has_auto_increment(&column.data_type))
}

fn parse_auto_increment_value(value: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|_| SqlRockError::new(format!("invalid AUTO_INCREMENT value `{value}`")))
}

fn increment_auto_value(value: u64) -> Result<u64> {
    value
        .checked_add(1)
        .ok_or_else(|| SqlRockError::new("AUTO_INCREMENT value is out of range"))
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

fn ensure_column_missing(table: &Table, column_name: &str, table_name: &str) -> Result<()> {
    if table
        .columns
        .iter()
        .any(|column| column.name.eq_ignore_ascii_case(column_name))
    {
        Err(SqlRockError::new(format!(
            "column `{column_name}` already exists for table `{table_name}`"
        )))
    } else {
        Ok(())
    }
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
