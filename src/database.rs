use crate::data_type::{
    has_auto_increment, has_default_current_timestamp, has_not_null,
    has_on_update_current_timestamp, has_primary_key, has_unique_key, is_timestamp_or_datetime,
    validate_auto_increment_columns, validate_key_columns,
};
use crate::datetime::now_string;
use crate::error::{Result, SqlRockError};
use crate::model::{
    AlterTableAction, Column, ForeignKey, ReferentialAction, SQL_NULL, SetClause, Statement, Table,
    WhereClause,
};
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
            Statement::CreateTable {
                name,
                columns,
                foreign_keys,
                comment,
                options,
                auto_increment_start,
            } => self.create_table(
                &name,
                columns,
                foreign_keys,
                comment,
                options,
                auto_increment_start,
            ),
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

    fn create_table(
        &self,
        name: &str,
        columns: Vec<Column>,
        foreign_keys: Vec<ForeignKey>,
        comment: Option<String>,
        options: Vec<crate::model::TableOption>,
        auto_increment_start: Option<u64>,
    ) -> Result<String> {
        fs::create_dir_all(&self.root)?;
        validate_auto_increment_columns(&columns)?;
        validate_key_columns(&columns)?;
        self.validate_foreign_key_definitions(name, &columns, &foreign_keys)?;

        let path = self.table_path(name)?;
        if path.exists() {
            return Err(SqlRockError::new(format!("table `{name}` already exists")));
        }

        let mut table = Table {
            name: name.to_string(),
            columns,
            foreign_keys,
            comment,
            options,
            auto_increment_next: auto_increment_start,
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
        apply_current_timestamp_defaults(&table, &mut row, &provided);
        validate_not_null_values(&table, &row, &provided)?;
        validate_key_values(&table, &row, None)?;
        normalize_null_values(&mut row);
        self.validate_foreign_key_values(&table, &row)?;
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
        let mut definitions = table
            .columns
            .iter()
            .map(|column| format!("{} {}", column.name, column.data_type))
            .collect::<Vec<_>>();
        definitions.extend(table.foreign_keys.iter().map(format_foreign_key));
        let columns = definitions.join(", ");
        let comment = table
            .comment
            .as_deref()
            .map(|comment| format!(" COMMENT='{}'", comment.replace('\'', "''")))
            .unwrap_or_default();
        let auto_increment = table
            .auto_increment_next
            .map(|value| format!(" AUTO_INCREMENT={value}"))
            .unwrap_or_default();
        let options = table
            .options
            .iter()
            .map(|option| format!(" {}={}", option.name, option.value))
            .collect::<String>();
        Ok(format!(
            "Table\tCreate Table\n{table_name}\tCREATE TABLE {table_name} ({columns}){comment}{auto_increment}{options}"
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
        validate_key_columns(&table.columns)?;
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
        self.validate_table_not_referenced(table_name)?;
        let path = self.existing_table_path(table_name)?;
        fs::remove_file(path)?;
        Ok(format!("dropped table `{table_name}`"))
    }

    fn delete_from(&self, table_name: &str, where_clause: WhereClause) -> Result<String> {
        let path = self.existing_table_path(table_name)?;
        let mut table = parse_table_file(&fs::read_to_string(&path)?)?;
        let column_index = column_index(&table, &where_clause.column, table_name)?;

        let original_len = table.rows.len();
        let deleted_rows = table
            .rows
            .iter()
            .filter(|row| row.get(column_index) == Some(&where_clause.value))
            .cloned()
            .collect::<Vec<_>>();
        self.apply_referencing_delete_actions(&table, &deleted_rows)?;
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
        self.apply_referencing_delete_actions(&table, &table.rows)?;
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
        self.apply_referencing_delete_actions(&table, &table.rows)?;
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

        let mut rows = table.rows.clone();
        let old_rows = rows.clone();
        let mut updated_count = 0;
        for row in &mut rows {
            if row.get(where_index) == Some(&where_clause.value) {
                let value = normalize_updated_value(&table.columns[set_index], &set_clause.value)?;
                row[set_index] = value;
                apply_on_update_current_timestamps(&table, row, &[set_index]);
                updated_count += 1;
            }
        }

        validate_all_key_values(&table, &rows)?;
        self.validate_foreign_key_rows(&table, &rows)?;
        self.apply_referencing_update_actions(&table, &old_rows, &rows)?;
        table.rows = rows;
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
        let mut rows = table.rows.clone();
        let old_rows = rows.clone();
        let mut updated_count = 0;
        for row in &mut rows {
            if row.get(where_index) == Some(&where_clause.value) {
                for (index, value) in &updates {
                    row[*index] = value.clone();
                }
                let explicit_indexes = updates.iter().map(|(index, _)| *index).collect::<Vec<_>>();
                apply_on_update_current_timestamps(&table, row, &explicit_indexes);
                updated_count += 1;
            }
        }
        validate_all_key_values(&table, &rows)?;
        self.validate_foreign_key_rows(&table, &rows)?;
        self.apply_referencing_update_actions(&table, &old_rows, &rows)?;
        table.rows = rows;
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

    fn table_paths(&self) -> Result<Vec<PathBuf>> {
        fs::create_dir_all(&self.root)?;
        let mut paths = fs::read_dir(&self.root)?
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

    fn read_all_tables(&self) -> Result<Vec<Table>> {
        self.table_paths()?
            .into_iter()
            .map(|path| parse_table_file(&fs::read_to_string(path)?))
            .collect()
    }

    fn validate_foreign_key_definitions(
        &self,
        table_name: &str,
        columns: &[Column],
        foreign_keys: &[ForeignKey],
    ) -> Result<()> {
        for foreign_key in foreign_keys {
            if foreign_key.columns.len() != foreign_key.referenced_columns.len() {
                return Err(SqlRockError::new(
                    "FOREIGN KEY column count must match referenced column count",
                ));
            }
            for column in &foreign_key.columns {
                if !columns
                    .iter()
                    .any(|item| item.name.eq_ignore_ascii_case(column))
                {
                    return Err(SqlRockError::new(format!(
                        "unknown column `{column}` in foreign key definition"
                    )));
                }
            }
            let referenced = self.read_table(&foreign_key.referenced_table)?;
            for column in &foreign_key.referenced_columns {
                column_index(&referenced, column, &foreign_key.referenced_table)?;
            }
            validate_referenced_key_columns(&referenced, &foreign_key.referenced_columns)?;
            if foreign_key.on_delete == ReferentialAction::SetNull
                || foreign_key.on_update == ReferentialAction::SetNull
            {
                for column in &foreign_key.columns {
                    let column = columns
                        .iter()
                        .find(|item| item.name.eq_ignore_ascii_case(column))
                        .expect("checked above");
                    if has_not_null(&column.data_type) {
                        return Err(SqlRockError::new(format!(
                            "foreign key column `{}` cannot use SET NULL because it is NOT NULL",
                            column.name
                        )));
                    }
                }
            }
            if foreign_key
                .referenced_table
                .eq_ignore_ascii_case(table_name)
            {
                return Err(SqlRockError::new(
                    "self-referencing FOREIGN KEY is not supported",
                ));
            }
        }
        Ok(())
    }

    fn validate_foreign_key_rows(&self, table: &Table, rows: &[Vec<String>]) -> Result<()> {
        for row in rows {
            self.validate_foreign_key_values(table, row)?;
        }
        Ok(())
    }

    fn validate_foreign_key_values(&self, table: &Table, row: &[String]) -> Result<()> {
        for foreign_key in &table.foreign_keys {
            let child_indexes = foreign_key_column_indexes(table, foreign_key)?;
            let key = child_indexes
                .iter()
                .map(|index| row.get(*index).cloned().unwrap_or_default())
                .collect::<Vec<_>>();
            if key.iter().any(|value| value.is_empty()) {
                continue;
            }

            let referenced = self.read_table(&foreign_key.referenced_table)?;
            let referenced_indexes =
                referenced_column_indexes(&referenced, &foreign_key.referenced_columns)?;
            if !referenced.rows.iter().any(|referenced_row| {
                referenced_indexes
                    .iter()
                    .zip(key.iter())
                    .all(|(index, value)| referenced_row.get(*index) == Some(value))
            }) {
                return Err(SqlRockError::new(format!(
                    "foreign key constraint fails on `{}`",
                    table.name
                )));
            }
        }
        Ok(())
    }

    fn validate_table_not_referenced(&self, table_name: &str) -> Result<()> {
        for table in self.read_all_tables()? {
            if table.name.eq_ignore_ascii_case(table_name) {
                continue;
            }
            if table.foreign_keys.iter().any(|foreign_key| {
                foreign_key
                    .referenced_table
                    .eq_ignore_ascii_case(table_name)
            }) {
                return Err(SqlRockError::new(format!(
                    "cannot drop table `{table_name}` because it is referenced by `{}`",
                    table.name
                )));
            }
        }
        Ok(())
    }

    fn apply_referencing_delete_actions(
        &self,
        parent: &Table,
        deleted_rows: &[Vec<String>],
    ) -> Result<()> {
        if deleted_rows.is_empty() {
            return Ok(());
        }

        for mut child in self.read_all_tables()? {
            if child.name.eq_ignore_ascii_case(&parent.name) {
                continue;
            }
            let mut changed = false;
            let foreign_keys = child.foreign_keys.clone();
            for foreign_key in foreign_keys.iter().filter(|foreign_key| {
                foreign_key
                    .referenced_table
                    .eq_ignore_ascii_case(&parent.name)
            }) {
                let child_indexes = foreign_key_column_indexes(&child, foreign_key)?;
                let parent_indexes =
                    referenced_column_indexes(parent, &foreign_key.referenced_columns)?;
                if !child.rows.iter().any(|child_row| {
                    deleted_rows.iter().any(|parent_row| {
                        row_references_parent(
                            child_row,
                            &child_indexes,
                            parent_row,
                            &parent_indexes,
                        )
                    })
                }) {
                    continue;
                }

                match foreign_key.on_delete {
                    ReferentialAction::Restrict | ReferentialAction::NoAction => {
                        return Err(SqlRockError::new(format!(
                            "cannot delete from `{}` because it is referenced by `{}`",
                            parent.name, child.name
                        )));
                    }
                    ReferentialAction::Cascade => {
                        child.rows.retain(|child_row| {
                            !deleted_rows.iter().any(|parent_row| {
                                row_references_parent(
                                    child_row,
                                    &child_indexes,
                                    parent_row,
                                    &parent_indexes,
                                )
                            })
                        });
                        changed = true;
                    }
                    ReferentialAction::SetNull => {
                        for child_row in &mut child.rows {
                            if deleted_rows.iter().any(|parent_row| {
                                row_references_parent(
                                    child_row,
                                    &child_indexes,
                                    parent_row,
                                    &parent_indexes,
                                )
                            }) {
                                for index in &child_indexes {
                                    child_row[*index].clear();
                                }
                                changed = true;
                            }
                        }
                    }
                }
            }
            if changed {
                fs::write(self.table_path(&child.name)?, serialize_table(&child))?;
            }
        }
        Ok(())
    }

    fn apply_referencing_update_actions(
        &self,
        parent: &Table,
        old_rows: &[Vec<String>],
        new_rows: &[Vec<String>],
    ) -> Result<()> {
        let updated_pairs = old_rows
            .iter()
            .zip(new_rows.iter())
            .filter(|(old_row, new_row)| old_row != new_row)
            .collect::<Vec<_>>();
        if updated_pairs.is_empty() {
            return Ok(());
        }

        for mut child in self.read_all_tables()? {
            if child.name.eq_ignore_ascii_case(&parent.name) {
                continue;
            }
            let mut changed = false;
            let foreign_keys = child.foreign_keys.clone();
            for foreign_key in foreign_keys.iter().filter(|foreign_key| {
                foreign_key
                    .referenced_table
                    .eq_ignore_ascii_case(&parent.name)
            }) {
                let child_indexes = foreign_key_column_indexes(&child, foreign_key)?;
                let parent_indexes =
                    referenced_column_indexes(parent, &foreign_key.referenced_columns)?;
                for (old_parent_row, new_parent_row) in &updated_pairs {
                    if parent_indexes
                        .iter()
                        .all(|index| old_parent_row.get(*index) == new_parent_row.get(*index))
                    {
                        continue;
                    }
                    if !child.rows.iter().any(|child_row| {
                        row_references_parent(
                            child_row,
                            &child_indexes,
                            old_parent_row,
                            &parent_indexes,
                        )
                    }) {
                        continue;
                    }

                    match foreign_key.on_update {
                        ReferentialAction::Restrict | ReferentialAction::NoAction => {
                            return Err(SqlRockError::new(format!(
                                "cannot update `{}` because it is referenced by `{}`",
                                parent.name, child.name
                            )));
                        }
                        ReferentialAction::Cascade => {
                            for child_row in &mut child.rows {
                                if row_references_parent(
                                    child_row,
                                    &child_indexes,
                                    old_parent_row,
                                    &parent_indexes,
                                ) {
                                    for (child_index, parent_index) in
                                        child_indexes.iter().zip(parent_indexes.iter())
                                    {
                                        child_row[*child_index] = new_parent_row
                                            .get(*parent_index)
                                            .cloned()
                                            .unwrap_or_default();
                                    }
                                    changed = true;
                                }
                            }
                        }
                        ReferentialAction::SetNull => {
                            for child_row in &mut child.rows {
                                if row_references_parent(
                                    child_row,
                                    &child_indexes,
                                    old_parent_row,
                                    &parent_indexes,
                                ) {
                                    for index in &child_indexes {
                                        child_row[*index].clear();
                                    }
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
            if changed {
                fs::write(self.table_path(&child.name)?, serialize_table(&child))?;
            }
        }
        Ok(())
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

fn apply_current_timestamp_defaults(table: &Table, row: &mut [String], provided: &[bool]) {
    let now = now_string();
    for (index, column) in table.columns.iter().enumerate() {
        if !provided[index]
            && is_timestamp_or_datetime(&column.data_type)
            && has_default_current_timestamp(&column.data_type)
        {
            row[index] = now.clone();
        }
    }
}

fn apply_on_update_current_timestamps(
    table: &Table,
    row: &mut [String],
    explicit_indexes: &[usize],
) {
    let now = now_string();
    for (index, column) in table.columns.iter().enumerate() {
        if !explicit_indexes.contains(&index)
            && is_timestamp_or_datetime(&column.data_type)
            && has_on_update_current_timestamp(&column.data_type)
        {
            row[index] = now.clone();
        }
    }
}

fn validate_not_null_values(table: &Table, row: &[String], provided: &[bool]) -> Result<()> {
    for (index, column) in table.columns.iter().enumerate() {
        let value = row.get(index);
        if has_not_null(&column.data_type)
            && (value.is_some_and(|value| value == SQL_NULL)
                || (!provided[index] && value.is_none_or(|value| value.is_empty())))
        {
            return Err(SqlRockError::new(format!(
                "column `{}` cannot be NULL",
                column.name
            )));
        }
    }
    Ok(())
}

fn validate_all_key_values(table: &Table, rows: &[Vec<String>]) -> Result<()> {
    for (row_index, row) in rows.iter().enumerate() {
        validate_key_values_in_rows(table, row, rows, Some(row_index))?;
    }
    Ok(())
}

fn validate_key_values(table: &Table, row: &[String], skip_row: Option<usize>) -> Result<()> {
    validate_key_values_in_rows(table, row, &table.rows, skip_row)
}

fn validate_key_values_in_rows(
    table: &Table,
    row: &[String],
    rows: &[Vec<String>],
    skip_row: Option<usize>,
) -> Result<()> {
    validate_primary_key_values_in_rows(table, row, rows, skip_row)?;

    for (index, column) in table.columns.iter().enumerate() {
        if !has_unique_key(&column.data_type) {
            continue;
        }

        let Some(value) = row.get(index) else {
            continue;
        };
        if has_unique_key(&column.data_type) && value == SQL_NULL {
            continue;
        }

        if rows.iter().enumerate().any(|(row_index, existing)| {
            Some(row_index) != skip_row && existing.get(index) == Some(value)
        }) {
            return Err(SqlRockError::new(format!(
                "duplicate value `{value}` for key `{}`",
                column.name
            )));
        }
    }

    Ok(())
}

fn validate_primary_key_values_in_rows(
    table: &Table,
    row: &[String],
    rows: &[Vec<String>],
    skip_row: Option<usize>,
) -> Result<()> {
    let primary_key_indexes = table
        .columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| has_primary_key(&column.data_type).then_some(index))
        .collect::<Vec<_>>();

    if primary_key_indexes.is_empty() {
        return Ok(());
    }

    let key_values = primary_key_indexes
        .iter()
        .filter_map(|index| row.get(*index))
        .cloned()
        .collect::<Vec<_>>();
    if key_values.len() != primary_key_indexes.len() {
        return Ok(());
    }

    if rows.iter().enumerate().any(|(row_index, existing)| {
        Some(row_index) != skip_row
            && primary_key_indexes
                .iter()
                .all(|index| existing.get(*index) == row.get(*index))
    }) {
        let key_name = primary_key_indexes
            .iter()
            .map(|index| table.columns[*index].name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(SqlRockError::new(format!(
            "duplicate value `{}` for key `{key_name}`",
            key_values.join(", ")
        )));
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

fn foreign_key_column_indexes(table: &Table, foreign_key: &ForeignKey) -> Result<Vec<usize>> {
    foreign_key
        .columns
        .iter()
        .map(|column| column_index(table, column, &table.name))
        .collect()
}

fn referenced_column_indexes(table: &Table, columns: &[String]) -> Result<Vec<usize>> {
    columns
        .iter()
        .map(|column| column_index(table, column, &table.name))
        .collect()
}

fn validate_referenced_key_columns(table: &Table, columns: &[String]) -> Result<()> {
    let indexes = referenced_column_indexes(table, columns)?;
    let all_primary = indexes
        .iter()
        .all(|index| has_primary_key(&table.columns[*index].data_type));
    let all_unique = indexes
        .iter()
        .all(|index| has_unique_key(&table.columns[*index].data_type));
    if all_primary || all_unique {
        Ok(())
    } else {
        Err(SqlRockError::new(format!(
            "referenced columns `{}` must be indexed",
            columns.join(", ")
        )))
    }
}

fn row_references_parent(
    child_row: &[String],
    child_indexes: &[usize],
    parent_row: &[String],
    parent_indexes: &[usize],
) -> bool {
    child_indexes
        .iter()
        .map(|index| child_row.get(*index).cloned().unwrap_or_default())
        .collect::<Vec<_>>()
        .iter()
        .all(|value| !value.is_empty())
        && child_indexes
            .iter()
            .zip(parent_indexes.iter())
            .all(|(child_index, parent_index)| {
                child_row.get(*child_index) == parent_row.get(*parent_index)
            })
}

fn format_foreign_key(foreign_key: &ForeignKey) -> String {
    let name = foreign_key
        .name
        .as_deref()
        .map(|name| format!("CONSTRAINT {name} "))
        .unwrap_or_default();
    let columns = foreign_key.columns.join(", ");
    let referenced_columns = foreign_key.referenced_columns.join(", ");
    let mut sql = format!(
        "{name}FOREIGN KEY ({columns}) REFERENCES {} ({referenced_columns})",
        foreign_key.referenced_table
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
