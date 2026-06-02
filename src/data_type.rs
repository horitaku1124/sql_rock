use crate::error::{Result, SqlRockError};

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
    let name = data_type
        .trim()
        .split(|ch: char| ch == '(' || ch.is_whitespace())
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();

    if SUPPORTED_DATA_TYPES.contains(&name.as_str()) {
        Ok(())
    } else {
        Err(SqlRockError::new(format!(
            "unsupported data type `{data_type}`"
        )))
    }
}
