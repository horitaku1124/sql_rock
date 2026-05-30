#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereClause {
    pub column: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetClause {
    pub column: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    CreateTable {
        name: String,
        columns: Vec<Column>,
    },
    InsertInto {
        table: String,
        columns: Vec<String>,
        values: Vec<String>,
    },
    SelectAll {
        table: String,
        where_clause: Option<WhereClause>,
    },
    SelectCount {
        table: String,
        column: String,
        where_clause: Option<WhereClause>,
    },
    DescribeTable {
        table: String,
    },
    DropTable {
        table: String,
    },
    DeleteFrom {
        table: String,
        where_clause: WhereClause,
    },
    Update {
        table: String,
        set_clause: SetClause,
        where_clause: WhereClause,
    },
}
