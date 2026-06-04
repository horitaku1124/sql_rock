use crate::datetime::{now_string, today_string};
use crate::error::{Result, SqlRockError};
use crate::model::{
    Aggregate, CompareOp, Condition, Expr, JoinKind, OrderBy, SelectItem, SelectQuery,
    SelectSource, Table,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl QueryResult {
    pub fn render(&self) -> String {
        let mut lines = vec![self.headers.join("\t")];
        lines.extend(self.rows.iter().map(|row| row.join("\t")));
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
struct DataSet {
    columns: Vec<ColumnBinding>,
    rows: Vec<Row>,
}

#[derive(Debug, Clone)]
struct ColumnBinding {
    key: String,
    label: String,
}

type Row = HashMap<String, String>;

pub fn execute_query(
    query: &SelectQuery,
    load_table: &impl Fn(&str) -> Result<Table>,
) -> Result<QueryResult> {
    evaluate_query(query, load_table, None)
}

fn evaluate_query(
    query: &SelectQuery,
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<QueryResult> {
    let mut data = evaluate_source(&query.source, load_table, outer_row)?;
    for join in &query.joins {
        let right = evaluate_source(&join.source, load_table, outer_row)?;
        data = join_data_sets(
            data,
            right,
            &join.kind,
            join.on.as_ref(),
            load_table,
            outer_row,
        )?;
    }

    if let Some(condition) = &query.where_clause {
        data.rows.retain(|row| {
            evaluate_condition(condition, row, &[], load_table, outer_row).unwrap_or(false)
        });
    }

    let grouped = !query.group_by.is_empty()
        || query
            .items
            .iter()
            .any(|item| contains_aggregate(&item.expr))
        || query
            .having
            .as_ref()
            .is_some_and(condition_contains_aggregate);
    let groups = if grouped {
        group_rows(&data.rows, &query.group_by, load_table, outer_row)?
    } else {
        data.rows.iter().map(|row| vec![row.clone()]).collect()
    };

    let headers = select_headers(&query.items, &data.columns);
    let mut projected = Vec::new();
    for group in groups {
        let row = group.first().cloned().unwrap_or_default();
        if query.having.as_ref().is_some_and(|condition| {
            !evaluate_condition(condition, &row, &group, load_table, outer_row).unwrap_or(false)
        }) {
            continue;
        }
        let values = project_row(
            &query.items,
            &data.columns,
            &row,
            &group,
            load_table,
            outer_row,
        )?;
        projected.push(ProjectedRow { values, row, group });
    }

    sort_rows(&mut projected, &query.order_by, load_table, outer_row)?;

    let mut rows = projected
        .into_iter()
        .map(|projected| projected.values)
        .collect::<Vec<_>>();
    if query.distinct {
        let mut seen = HashSet::new();
        rows.retain(|row| seen.insert(row.clone()));
    }
    rows = rows
        .into_iter()
        .skip(query.offset)
        .take(query.limit.unwrap_or(usize::MAX))
        .collect();

    let mut result = QueryResult { headers, rows };
    if let Some((all, union_query)) = &query.union {
        let union_result = evaluate_query(union_query, load_table, outer_row)?;
        if result.headers.len() != union_result.headers.len() {
            return Err(SqlRockError::new(
                "UNION queries must return the same number of columns",
            ));
        }
        result.rows.extend(union_result.rows);
        if !all {
            let mut seen = HashSet::new();
            result.rows.retain(|row| seen.insert(row.clone()));
        }
    }
    Ok(result)
}

fn evaluate_source(
    source: &SelectSource,
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<DataSet> {
    match source {
        SelectSource::Table { name, alias } => {
            let table = load_table(name)?;
            let columns = table
                .columns
                .iter()
                .map(|column| ColumnBinding {
                    key: format!("{alias}.{}", column.name),
                    label: column.name.clone(),
                })
                .collect::<Vec<_>>();
            let rows = table
                .rows
                .iter()
                .map(|values| {
                    let mut row = Row::new();
                    for (column, value) in table.columns.iter().zip(values) {
                        row.insert(format!("{alias}.{}", column.name), value.clone());
                        row.entry(column.name.clone())
                            .or_insert_with(|| value.clone());
                    }
                    row
                })
                .collect();
            Ok(DataSet { columns, rows })
        }
        SelectSource::Subquery { query, alias } => {
            let result = evaluate_query(query, load_table, outer_row)?;
            let columns = result
                .headers
                .iter()
                .map(|header| ColumnBinding {
                    key: format!("{alias}.{header}"),
                    label: header.clone(),
                })
                .collect::<Vec<_>>();
            let rows = result
                .rows
                .iter()
                .map(|values| {
                    let mut row = Row::new();
                    for (header, value) in result.headers.iter().zip(values) {
                        row.insert(format!("{alias}.{header}"), value.clone());
                        row.entry(header.clone()).or_insert_with(|| value.clone());
                    }
                    row
                })
                .collect();
            Ok(DataSet { columns, rows })
        }
    }
}

fn join_data_sets(
    left: DataSet,
    right: DataSet,
    kind: &JoinKind,
    condition: Option<&Condition>,
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<DataSet> {
    let mut rows = Vec::new();
    let mut matched_right = vec![false; right.rows.len()];
    for left_row in &left.rows {
        let mut matched = false;
        for (right_index, right_row) in right.rows.iter().enumerate() {
            let merged = merge_rows(left_row, right_row);
            let is_match = condition
                .map(|condition| evaluate_condition(condition, &merged, &[], load_table, outer_row))
                .transpose()?
                .unwrap_or(true);
            if is_match {
                matched = true;
                matched_right[right_index] = true;
                rows.push(merged);
            }
        }
        if !matched && *kind == JoinKind::Left {
            rows.push(merge_rows(left_row, &empty_row(&right.columns)));
        }
    }
    if *kind == JoinKind::Right {
        for (right_index, right_row) in right.rows.iter().enumerate() {
            if !matched_right[right_index] {
                rows.push(merge_rows(&empty_row(&left.columns), right_row));
            }
        }
    }
    Ok(DataSet {
        columns: left.columns.into_iter().chain(right.columns).collect(),
        rows,
    })
}

fn project_row(
    items: &[SelectItem],
    columns: &[ColumnBinding],
    row: &Row,
    group: &[Row],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<Vec<String>> {
    let mut values = Vec::new();
    for item in items {
        if item.expr == Expr::All {
            values.extend(columns.iter().map(|column| {
                row.get(&column.key)
                    .or_else(|| row.get(&column.label))
                    .cloned()
                    .unwrap_or_default()
            }));
        } else {
            values.push(evaluate_expr(
                &item.expr, row, group, load_table, outer_row,
            )?);
        }
    }
    Ok(values)
}

fn evaluate_expr(
    expr: &Expr,
    row: &Row,
    group: &[Row],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<String> {
    match expr {
        Expr::All => Ok("*".to_string()),
        Expr::Column(column) => row
            .get(column)
            .or_else(|| outer_row.and_then(|outer| outer.get(column)))
            .cloned()
            .ok_or_else(|| SqlRockError::new(format!("unknown column `{column}`"))),
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Now => Ok(now_string()),
        Expr::Today => Ok(today_string()),
        Expr::Aggregate(aggregate, expr) => {
            evaluate_aggregate(aggregate, expr, group, load_table, outer_row)
        }
        Expr::Case { branches, fallback } => {
            for (condition, value) in branches {
                if evaluate_condition(condition, row, group, load_table, outer_row)? {
                    return evaluate_expr(value, row, group, load_table, outer_row);
                }
            }
            evaluate_expr(fallback, row, group, load_table, outer_row)
        }
    }
}

fn evaluate_aggregate(
    aggregate: &Aggregate,
    expr: &Expr,
    group: &[Row],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<String> {
    let values = if *expr == Expr::All {
        vec!["1".to_string(); group.len()]
    } else {
        group
            .iter()
            .map(|row| evaluate_expr(expr, row, &[], load_table, outer_row))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect()
    };
    match aggregate {
        Aggregate::Count => Ok(values.len().to_string()),
        Aggregate::Sum => Ok(format_number(
            values
                .iter()
                .map(|value| parse_number(value))
                .sum::<Result<f64>>()?,
        )),
        Aggregate::Avg => {
            if values.is_empty() {
                Ok(String::new())
            } else {
                Ok(format_number(
                    values
                        .iter()
                        .map(|value| parse_number(value))
                        .sum::<Result<f64>>()?
                        / values.len() as f64,
                ))
            }
        }
        Aggregate::Max => Ok(values
            .into_iter()
            .max_by(|left, right| compare_values(left, right))
            .unwrap_or_default()),
        Aggregate::Min => Ok(values
            .into_iter()
            .min_by(|left, right| compare_values(left, right))
            .unwrap_or_default()),
    }
}

fn evaluate_condition(
    condition: &Condition,
    row: &Row,
    group: &[Row],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<bool> {
    match condition {
        Condition::Compare(left, operator, right) => {
            let left = evaluate_expr(left, row, group, load_table, outer_row)?;
            let right = evaluate_expr(right, row, group, load_table, outer_row)?;
            Ok(compare(&left, operator, &right))
        }
        Condition::Between(expr, start, end) => {
            let value = evaluate_expr(expr, row, group, load_table, outer_row)?;
            let start = evaluate_expr(start, row, group, load_table, outer_row)?;
            let end = evaluate_expr(end, row, group, load_table, outer_row)?;
            Ok(compare(&value, &CompareOp::GreaterEq, &start)
                && compare(&value, &CompareOp::LessEq, &end))
        }
        Condition::InValues(expr, values) => {
            let value = evaluate_expr(expr, row, group, load_table, outer_row)?;
            Ok(values
                .iter()
                .map(|candidate| evaluate_expr(candidate, row, group, load_table, outer_row))
                .collect::<Result<Vec<_>>>()?
                .contains(&value))
        }
        Condition::InQuery(expr, query) => {
            let value = evaluate_expr(expr, row, group, load_table, outer_row)?;
            let result = evaluate_query(query, load_table, Some(row))?;
            Ok(result
                .rows
                .iter()
                .any(|candidate| candidate.first() == Some(&value)))
        }
        Condition::Like(expr, pattern) => Ok(like(
            &evaluate_expr(expr, row, group, load_table, outer_row)?,
            pattern,
        )),
        Condition::IsNull(expr, not) => {
            let is_null = evaluate_expr(expr, row, group, load_table, outer_row)?.is_empty();
            Ok(if *not { !is_null } else { is_null })
        }
        Condition::Exists(query) => Ok(!evaluate_query(query, load_table, Some(row))?
            .rows
            .is_empty()),
        Condition::And(left, right) => {
            Ok(evaluate_condition(left, row, group, load_table, outer_row)?
                && evaluate_condition(right, row, group, load_table, outer_row)?)
        }
        Condition::Or(left, right) => {
            Ok(evaluate_condition(left, row, group, load_table, outer_row)?
                || evaluate_condition(right, row, group, load_table, outer_row)?)
        }
    }
}

fn group_rows(
    rows: &[Row],
    group_by: &[Expr],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<Vec<Vec<Row>>> {
    if group_by.is_empty() {
        return Ok(vec![rows.to_vec()]);
    }
    let mut groups = Vec::<(Vec<String>, Vec<Row>)>::new();
    for row in rows {
        let key = group_by
            .iter()
            .map(|expr| evaluate_expr(expr, row, &[], load_table, outer_row))
            .collect::<Result<Vec<_>>>()?;
        if let Some((_, group)) = groups.iter_mut().find(|(existing, _)| *existing == key) {
            group.push(row.clone());
        } else {
            groups.push((key, vec![row.clone()]));
        }
    }
    Ok(groups.into_iter().map(|(_, rows)| rows).collect())
}

fn sort_rows(
    rows: &mut [ProjectedRow],
    order_by: &[OrderBy],
    load_table: &impl Fn(&str) -> Result<Table>,
    outer_row: Option<&Row>,
) -> Result<()> {
    let mut keys = Vec::new();
    for row in rows.iter() {
        keys.push(
            order_by
                .iter()
                .map(|order| {
                    evaluate_expr(&order.expr, &row.row, &row.group, load_table, outer_row)
                })
                .collect::<Result<Vec<_>>>()?,
        );
    }
    let mut indexed = rows.iter().cloned().zip(keys).collect::<Vec<_>>();
    indexed.sort_by(|(_, left), (_, right)| {
        for ((left, right), order) in left.iter().zip(right).zip(order_by) {
            let ordering = compare_values(left, right);
            if ordering != Ordering::Equal {
                return if order.descending {
                    ordering.reverse()
                } else {
                    ordering
                };
            }
        }
        Ordering::Equal
    });
    for (target, (row, _)) in rows.iter_mut().zip(indexed) {
        *target = row;
    }
    Ok(())
}

#[derive(Clone)]
struct ProjectedRow {
    values: Vec<String>,
    row: Row,
    group: Vec<Row>,
}

fn select_headers(items: &[SelectItem], columns: &[ColumnBinding]) -> Vec<String> {
    let mut headers = Vec::new();
    for item in items {
        if item.expr == Expr::All {
            headers.extend(columns.iter().map(|column| column.label.clone()));
        } else {
            headers.push(item.alias.clone().unwrap_or_else(|| expr_label(&item.expr)));
        }
    }
    headers
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::All => "*".to_string(),
        Expr::Column(column) => column.clone(),
        Expr::Literal(value) => value.clone(),
        Expr::Now => "now()".to_string(),
        Expr::Today => "today()".to_string(),
        Expr::Aggregate(aggregate, expr) => {
            format!("{}({})", aggregate_label(aggregate), expr_label(expr))
        }
        Expr::Case { .. } => "case".to_string(),
    }
}

fn aggregate_label(aggregate: &Aggregate) -> &'static str {
    match aggregate {
        Aggregate::Count => "count",
        Aggregate::Sum => "sum",
        Aggregate::Avg => "avg",
        Aggregate::Max => "max",
        Aggregate::Min => "min",
    }
}

fn contains_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate(_, _) => true,
        Expr::Case { branches, fallback } => {
            branches.iter().any(|(_, expr)| contains_aggregate(expr))
                || contains_aggregate(fallback)
        }
        _ => false,
    }
}

fn condition_contains_aggregate(condition: &Condition) -> bool {
    match condition {
        Condition::Compare(left, _, right) | Condition::Between(left, right, _) => {
            contains_aggregate(left) || contains_aggregate(right)
        }
        Condition::InValues(expr, values) => {
            contains_aggregate(expr) || values.iter().any(contains_aggregate)
        }
        Condition::Like(expr, _) | Condition::IsNull(expr, _) => contains_aggregate(expr),
        Condition::And(left, right) | Condition::Or(left, right) => {
            condition_contains_aggregate(left) || condition_contains_aggregate(right)
        }
        Condition::InQuery(_, _) | Condition::Exists(_) => false,
    }
}

fn compare(left: &str, operator: &CompareOp, right: &str) -> bool {
    let ordering = compare_values(left, right);
    match operator {
        CompareOp::Eq => ordering == Ordering::Equal,
        CompareOp::NotEq => ordering != Ordering::Equal,
        CompareOp::Greater => ordering == Ordering::Greater,
        CompareOp::Less => ordering == Ordering::Less,
        CompareOp::GreaterEq => ordering != Ordering::Less,
        CompareOp::LessEq => ordering != Ordering::Greater,
    }
}

fn compare_values(left: &str, right: &str) -> Ordering {
    match (left.parse::<f64>(), right.parse::<f64>()) {
        (Ok(left), Ok(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        _ => left.cmp(right),
    }
}

fn parse_number(value: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| SqlRockError::new(format!("expected numeric value, got `{value}`")))
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn merge_rows(left: &Row, right: &Row) -> Row {
    let mut row = left.clone();
    for (key, value) in right {
        if key.contains('.') || !row.contains_key(key) {
            row.insert(key.clone(), value.clone());
        }
    }
    row
}

fn empty_row(columns: &[ColumnBinding]) -> Row {
    let mut row = Row::new();
    for column in columns {
        row.insert(column.key.clone(), String::new());
        row.entry(column.label.clone()).or_default();
    }
    row
}

fn like(value: &str, pattern: &str) -> bool {
    fn matches(value: &[char], pattern: &[char]) -> bool {
        match pattern {
            [] => value.is_empty(),
            ['%', rest @ ..] => {
                matches(value, rest) || (!value.is_empty() && matches(&value[1..], pattern))
            }
            ['_', rest @ ..] => !value.is_empty() && matches(&value[1..], rest),
            [expected, rest @ ..] => value.first() == Some(expected) && matches(&value[1..], rest),
        }
    }
    matches(
        &value.chars().collect::<Vec<_>>(),
        &pattern.chars().collect::<Vec<_>>(),
    )
}
