use crate::error::{Result, SqlRockError};
use crate::model::{Column, Table};

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
    let Some(rows_marker) = lines.next() else {
        return Err(SqlRockError::new("table file is missing rows marker"));
    };

    let name = table_line
        .strip_prefix("table:")
        .ok_or_else(|| SqlRockError::new("invalid table line"))?;
    let columns = columns_line
        .strip_prefix("columns:")
        .ok_or_else(|| SqlRockError::new("invalid columns line"))?;
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
        rows,
    })
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
