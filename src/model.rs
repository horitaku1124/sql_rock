pub const SQL_NULL: &str = "\0NULL";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub data_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub auto_increment_next: Option<u64>,
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
pub struct SelectQuery {
    pub distinct: bool,
    pub items: Vec<SelectItem>,
    pub source: SelectSource,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Condition>,
    pub group_by: Vec<Expr>,
    pub having: Option<Condition>,
    pub order_by: Vec<OrderBy>,
    pub limit: Option<usize>,
    pub offset: usize,
    pub union: Option<(bool, Box<SelectQuery>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectSource {
    Table {
        name: String,
        alias: String,
    },
    Subquery {
        query: Box<SelectQuery>,
        alias: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinClause {
    pub kind: JoinKind,
    pub source: SelectSource,
    pub on: Option<Condition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Cross,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectItem {
    pub expr: Expr,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderBy {
    pub expr: Expr,
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    All,
    Column(String),
    Literal(String),
    Aggregate(Aggregate, Box<Expr>),
    Case {
        branches: Vec<(Condition, Box<Expr>)>,
        fallback: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aggregate {
    Count,
    Sum,
    Avg,
    Max,
    Min,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Compare(Expr, CompareOp, Expr),
    Between(Expr, Expr, Expr),
    InValues(Expr, Vec<Expr>),
    InQuery(Expr, Box<SelectQuery>),
    Like(Expr, String),
    IsNull(Expr, bool),
    Exists(Box<SelectQuery>),
    And(Box<Condition>, Box<Condition>),
    Or(Box<Condition>, Box<Condition>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    NotEq,
    Greater,
    Less,
    GreaterEq,
    LessEq,
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
    InsertRows {
        table: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    InsertSelect {
        table: String,
        query: Box<SelectQuery>,
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
    ShowTables,
    ShowCreateTable {
        table: String,
    },
    AlterTable {
        table: String,
        action: AlterTableAction,
    },
    DropTable {
        table: String,
    },
    DeleteFrom {
        table: String,
        where_clause: WhereClause,
    },
    DeleteAll {
        table: String,
    },
    TruncateTable {
        table: String,
    },
    Update {
        table: String,
        set_clause: SetClause,
        where_clause: WhereClause,
    },
    UpdateMany {
        table: String,
        set_clauses: Vec<SetClause>,
        where_clause: WhereClause,
    },
    SelectQuery(Box<SelectQuery>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlterTableAction {
    Add(Column),
    Modify(Column),
    Change { old_name: String, column: Column },
}
