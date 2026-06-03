use crate::error::{Result, SqlRockError};
use crate::model::Column;

const SUPPORTED_DATA_TYPES: &[&str] = &[
    "TINYINT",
    "SMALLINT",
    "MEDIUMINT",
    "INT",
    "INTEGER",
    "BIGINT",
    "DECIMAL",
    "NUMERIC",
    "DEC",
    "FIXED",
    "FLOAT",
    "DOUBLE",
    "REAL",
    "BIT",
    "DATE",
    "TIME",
    "DATETIME",
    "TIMESTAMP",
    "YEAR",
    "CHAR",
    "VARCHAR",
    "BINARY",
    "VARBINARY",
    "TINYBLOB",
    "BLOB",
    "MEDIUMBLOB",
    "LONGBLOB",
    "TINYTEXT",
    "TEXT",
    "MEDIUMTEXT",
    "LONGTEXT",
    "ENUM",
    "SET",
    "VECTOR",
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

pub fn validate_data_type(data_type: &str) -> Result<()> {
    let name = data_type_name(data_type);

    if SUPPORTED_DATA_TYPES.contains(&name.as_str()) {
        Ok(())
    } else {
        Err(SqlRockError::new(format!(
            "unsupported data type `{data_type}`"
        )))
    }
}

pub fn validate_auto_increment_columns(columns: &[Column]) -> Result<()> {
    let auto_increment_columns = columns
        .iter()
        .filter(|column| has_auto_increment(&column.data_type))
        .collect::<Vec<_>>();

    if auto_increment_columns.len() > 1 {
        return Err(SqlRockError::new(
            "only one AUTO_INCREMENT column is allowed",
        ));
    }

    if let Some(column) = auto_increment_columns.first()
        && !is_integer_type(&column.data_type)
    {
        return Err(SqlRockError::new(format!(
            "AUTO_INCREMENT column `{}` requires an integer data type",
            column.name
        )));
    }

    Ok(())
}

pub fn has_auto_increment(data_type: &str) -> bool {
    data_type
        .split_whitespace()
        .any(|part| part.eq_ignore_ascii_case("AUTO_INCREMENT"))
}

pub fn has_not_null(data_type: &str) -> bool {
    let parts = data_type.split_whitespace().collect::<Vec<_>>();
    parts.windows(2).any(|window| {
        window[0].eq_ignore_ascii_case("NOT") && window[1].eq_ignore_ascii_case("NULL")
    })
}

fn data_type_name(data_type: &str) -> String {
    data_type
        .trim()
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase()
}

fn is_integer_type(data_type: &str) -> bool {
    matches!(
        data_type_name(data_type).as_str(),
        "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "INTEGER" | "BIGINT"
    )
}
