use crate::error::{Result, SqlRockError};
use crate::model::{Column, ForeignKey, ReferentialAction, Table, TableOption};

pub fn serialize_table(table: &Table) -> String {
    let mut lines = Vec::new();
    lines.push(format!("table:{}", escape_field(&table.name)));
    lines.push(format!(
        "columns:{}",
        table
            .columns
            .iter()
            .map(|column| format!(
                "{}:{}",
                escape_field(&column.name),
                escape_field(&column.data_type)
            ))
            .collect::<Vec<_>>()
            .join("|")
    ));
    lines.push(format!(
        "foreign_keys:{}",
        table
            .foreign_keys
            .iter()
            .map(|foreign_key| {
                [
                    foreign_key
                        .name
                        .as_deref()
                        .map(escape_field)
                        .unwrap_or_default(),
                    foreign_key
                        .columns
                        .iter()
                        .map(|column| escape_field(column))
                        .collect::<Vec<_>>()
                        .join(","),
                    escape_field(&foreign_key.referenced_table),
                    foreign_key
                        .referenced_columns
                        .iter()
                        .map(|column| escape_field(column))
                        .collect::<Vec<_>>()
                        .join(","),
                    referential_action_label(foreign_key.on_delete).to_string(),
                    referential_action_label(foreign_key.on_update).to_string(),
                ]
                .join(":")
            })
            .collect::<Vec<_>>()
            .join("|")
    ));
    lines.push(format!(
        "comment:{}",
        table
            .comment
            .as_deref()
            .map(escape_field)
            .unwrap_or_default()
    ));
    lines.push(format!(
        "options:{}",
        table
            .options
            .iter()
            .map(|option| format!(
                "{}:{}",
                escape_field(&option.name),
                escape_field(&option.value)
            ))
            .collect::<Vec<_>>()
            .join("|")
    ));
    lines.push(format!(
        "auto_increment:{}",
        table
            .auto_increment_next
            .map(|value| value.to_string())
            .unwrap_or_default()
    ));
    lines.push("rows:".to_string());
    lines.extend(table.rows.iter().map(|row| {
        row.iter()
            .map(|field| escape_field(field))
            .collect::<Vec<_>>()
            .join("|")
    }));
    lines.push(String::new());
    lines.join("\n")
}

pub fn parse_table_file(content: &str) -> Result<Table> {
    let mut lines = content.lines();
    let Some(table_line) = lines.next() else {
        return Err(SqlRockError::new("table file is empty"));
    };
    let Some(columns_line) = lines.next() else {
        return Err(SqlRockError::new("table file is missing columns"));
    };
    let Some(metadata_or_rows_marker) = lines.next() else {
        return Err(SqlRockError::new("table file is missing rows marker"));
    };

    let name = table_line
        .strip_prefix("table:")
        .ok_or_else(|| SqlRockError::new("invalid table line"))?;
    let columns = columns_line
        .strip_prefix("columns:")
        .ok_or_else(|| SqlRockError::new("invalid columns line"))?;
    let mut foreign_keys = Vec::new();
    let mut comment = None;
    let mut options = Vec::new();
    let mut metadata_or_rows_marker = metadata_or_rows_marker;
    if let Some(value) = metadata_or_rows_marker.strip_prefix("foreign_keys:") {
        if !value.is_empty() {
            foreign_keys = value
                .split('|')
                .map(parse_foreign_key_metadata)
                .collect::<Result<Vec<_>>>()?;
        }
        metadata_or_rows_marker = lines
            .next()
            .ok_or_else(|| SqlRockError::new("table file is missing rows marker"))?;
    }
    if let Some(value) = metadata_or_rows_marker.strip_prefix("comment:") {
        if !value.is_empty() {
            comment = Some(unescape_field(value)?);
        }
        metadata_or_rows_marker = lines
            .next()
            .ok_or_else(|| SqlRockError::new("table file is missing rows marker"))?;
    }
    if let Some(value) = metadata_or_rows_marker.strip_prefix("options:") {
        if !value.is_empty() {
            options = value
                .split('|')
                .map(|item| {
                    let Some((name, value)) = item.split_once(':') else {
                        return Err(SqlRockError::new("invalid table option metadata"));
                    };
                    Ok(TableOption {
                        name: unescape_field(name)?,
                        value: unescape_field(value)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
        }
        metadata_or_rows_marker = lines
            .next()
            .ok_or_else(|| SqlRockError::new("table file is missing rows marker"))?;
    }

    let (auto_increment_next, rows_marker) =
        if let Some(value) = metadata_or_rows_marker.strip_prefix("auto_increment:") {
            let next = if value.is_empty() {
                None
            } else {
                Some(
                    value
                        .parse()
                        .map_err(|_| SqlRockError::new("invalid auto increment value"))?,
                )
            };
            let rows_marker = lines
                .next()
                .ok_or_else(|| SqlRockError::new("table file is missing rows marker"))?;
            (next, rows_marker)
        } else {
            (None, metadata_or_rows_marker)
        };
    if rows_marker != "rows:" {
        return Err(SqlRockError::new("invalid rows marker"));
    }

    let columns = if columns.is_empty() {
        Vec::new()
    } else {
        columns
            .split('|')
            .map(|item| {
                let Some((name, data_type)) = item.split_once(':') else {
                    return Err(SqlRockError::new("invalid column metadata"));
                };
                Ok(Column {
                    name: unescape_field(name)?,
                    data_type: unescape_field(data_type)?,
                })
            })
            .collect::<Result<Vec<_>>>()?
    };

    let mut rows = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }

        let row = line
            .split('|')
            .map(unescape_field)
            .collect::<Result<Vec<_>>>()?;
        if row.len() != columns.len() {
            return Err(SqlRockError::new("row width does not match table columns"));
        }
        rows.push(row);
    }

    Ok(Table {
        name: unescape_field(name)?,
        columns,
        foreign_keys,
        comment,
        options,
        auto_increment_next,
        rows,
    })
}

fn parse_foreign_key_metadata(value: &str) -> Result<ForeignKey> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 6 {
        return Err(SqlRockError::new("invalid foreign key metadata"));
    }

    Ok(ForeignKey {
        name: if parts[0].is_empty() {
            None
        } else {
            Some(unescape_field(parts[0])?)
        },
        columns: split_metadata_columns(parts[1])?,
        referenced_table: unescape_field(parts[2])?,
        referenced_columns: split_metadata_columns(parts[3])?,
        on_delete: parse_referential_action(parts[4])?,
        on_update: parse_referential_action(parts[5])?,
    })
}

fn split_metadata_columns(value: &str) -> Result<Vec<String>> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(unescape_field)
        .collect::<Result<Vec<_>>>()
}

fn referential_action_label(action: ReferentialAction) -> &'static str {
    match action {
        ReferentialAction::Restrict => "RESTRICT",
        ReferentialAction::Cascade => "CASCADE",
        ReferentialAction::SetNull => "SET NULL",
        ReferentialAction::NoAction => "NO ACTION",
    }
}

fn parse_referential_action(value: &str) -> Result<ReferentialAction> {
    match value {
        "RESTRICT" => Ok(ReferentialAction::Restrict),
        "CASCADE" => Ok(ReferentialAction::Cascade),
        "SET NULL" => Ok(ReferentialAction::SetNull),
        "NO ACTION" => Ok(ReferentialAction::NoAction),
        _ => Err(SqlRockError::new("invalid referential action metadata")),
    }
}

fn escape_field(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('|', "\\p")
        .replace(':', "\\c")
        .replace('\n', "\\n")
}

fn unescape_field(value: &str) -> Result<String> {
    let mut result = String::new();
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            result.push(ch);
            continue;
        }

        let Some(escaped) = chars.next() else {
            return Err(SqlRockError::new("invalid escape sequence"));
        };
        match escaped {
            '\\' => result.push('\\'),
            'p' => result.push('|'),
            'c' => result.push(':'),
            'n' => result.push('\n'),
            other => {
                return Err(SqlRockError::new(format!(
                    "unknown escape sequence `\\{other}`"
                )));
            }
        }
    }

    Ok(result)
}
