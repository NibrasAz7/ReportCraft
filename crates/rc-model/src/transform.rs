//! `transform` — JSON record connectors, Power Query step-based data
//! transformations, row-context calculated columns, and SQL query folding.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{parse_table_col, Column, ModelError, Scalar, Table, MAX_COLS, MAX_ROWS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
}

impl FilterOp {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "=" | "==" | "eq" => Some(FilterOp::Eq),
            "!=" | "<>" | "ne" => Some(FilterOp::Ne),
            ">" | "gt" => Some(FilterOp::Gt),
            ">=" | "ge" => Some(FilterOp::Ge),
            "<" | "lt" => Some(FilterOp::Lt),
            "<=" | "le" => Some(FilterOp::Le),
            "contains" | "like" => Some(FilterOp::Contains),
            _ => None,
        }
    }

    pub fn sql_op(self) -> &'static str {
        match self {
            FilterOp::Eq => "=",
            FilterOp::Ne => "<>",
            FilterOp::Gt => ">",
            FilterOp::Ge => ">=",
            FilterOp::Lt => "<",
            FilterOp::Le => "<=",
            FilterOp::Contains => "LIKE",
        }
    }
}

/// A step in a Power Query-style table transformation pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum TransformStep {
    FilterRows { column: String, op: FilterOp, value: String },
    SelectColumns { columns: Vec<String> },
    RenameColumn { from: String, to: String },
    AddCalculatedColumn { name: String, formula: String },
    FillNull { column: String, replacement: String },
}

impl Table {
    /// Parse a JSON array of flat objects (`[{"a": 1, "b": "x"}, ...]`) into a columnar `Table`.
    pub fn from_json_records(name: impl Into<String>, json_text: &str) -> Result<Self, ModelError> {
        let parsed: Value = serde_json::from_str(json_text).map_err(|e| ModelError::Serialization(e.to_string()))?;
        let Value::Array(rows) = parsed else {
            return Err(ModelError::Serialization("expected a JSON array of row objects".into()));
        };
        if rows.is_empty() {
            return Err(ModelError::EmptyTable);
        }
        if rows.len() > MAX_ROWS {
            return Err(ModelError::RowLimitExceeded(rows.len()));
        }

        let mut col_names: Vec<String> = Vec::new();
        for row in &rows {
            let Value::Object(map) = row else {
                return Err(ModelError::Serialization("each JSON row must be an object".into()));
            };
            for key in map.keys() {
                if !col_names.iter().any(|k| k == key) {
                    col_names.push(key.clone());
                }
            }
        }
        if col_names.is_empty() {
            return Err(ModelError::EmptyTable);
        }
        if col_names.len() > MAX_COLS {
            return Err(ModelError::ColumnLimitExceeded(col_names.len()));
        }

        let mut columns: Vec<Column> = col_names.iter().map(|c| Column { name: c.clone(), values: Vec::with_capacity(rows.len()) }).collect();

        for row in &rows {
            let Value::Object(map) = row else { continue };
            for col in &mut columns {
                let scalar = match map.get(&col.name) {
                    None | Some(Value::Null) => Scalar::Null,
                    Some(Value::Bool(b)) => Scalar::Bool(*b),
                    Some(Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            Scalar::Int(i)
                        } else if let Some(f) = n.as_f64() {
                            Scalar::Float(f)
                        } else {
                            Scalar::Null
                        }
                    }
                    Some(Value::String(s)) => Scalar::parse_auto(s),
                    Some(other) => Scalar::Text(other.to_string()),
                };
                col.values.push(scalar);
            }
        }

        Ok(Table { name: name.into(), columns })
    }

    /// Export this `Table` as a pretty-printed JSON array of objects.
    pub fn to_json_records(&self) -> Result<String, ModelError> {
        let nrows = self.row_count();
        let mut arr = Vec::with_capacity(nrows);
        for r in 0..nrows {
            let mut obj = serde_json::Map::new();
            for col in &self.columns {
                let val = match col.values.get(r).unwrap_or(&Scalar::Null) {
                    Scalar::Null => Value::Null,
                    Scalar::Bool(b) => Value::Bool(*b),
                    Scalar::Int(i) => Value::from(*i),
                    Scalar::Float(f) => Value::from(*f),
                    Scalar::Text(s) => Value::String(s.clone()),
                };
                obj.insert(col.name.clone(), val);
            }
            arr.push(Value::Object(obj));
        }
        serde_json::to_string_pretty(&Value::Array(arr)).map_err(|e| ModelError::Serialization(e.to_string()))
    }

    pub fn apply_steps(&mut self, steps: &[TransformStep]) -> Result<(), ModelError> {
        for s in steps {
            self.apply_step(s)?;
        }
        Ok(())
    }

    pub fn apply_step(&mut self, step: &TransformStep) -> Result<(), ModelError> {
        match step {
            TransformStep::FilterRows { column, op, value } => {
                let col = self.column(column).ok_or_else(|| ModelError::UnknownColumn { table: self.name.clone(), column: column.clone() })?;
                let target_num = value.trim().parse::<f64>().ok();
                let target_str = value.trim();
                let nrows = self.row_count();
                let mut keep = Vec::with_capacity(nrows);
                for r in 0..nrows {
                    let cell = col.values.get(r).unwrap_or(&Scalar::Null);
                    let matched = match (cell.as_f64(), target_num) {
                        (Some(a), Some(b)) => match op {
                            FilterOp::Eq => (a - b).abs() < 1e-9,
                            FilterOp::Ne => (a - b).abs() >= 1e-9,
                            FilterOp::Gt => a > b,
                            FilterOp::Ge => a >= b,
                            FilterOp::Lt => a < b,
                            FilterOp::Le => a <= b,
                            FilterOp::Contains => cell.display_key().contains(target_str),
                        },
                        _ => {
                            let s = cell.display_key();
                            match op {
                                FilterOp::Eq => s == target_str,
                                FilterOp::Ne => s != target_str,
                                FilterOp::Gt => s.as_str() > target_str,
                                FilterOp::Ge => s.as_str() >= target_str,
                                FilterOp::Lt => s.as_str() < target_str,
                                FilterOp::Le => s.as_str() <= target_str,
                                FilterOp::Contains => s.to_ascii_lowercase().contains(&target_str.to_ascii_lowercase()),
                            }
                        }
                    };
                    if matched {
                        keep.push(r);
                    }
                }
                for c in &mut self.columns {
                    c.values = keep.iter().filter_map(|&idx| c.values.get(idx).cloned()).collect();
                }
                Ok(())
            }
            TransformStep::SelectColumns { columns } => {
                for wanted in columns {
                    if self.column(wanted).is_none() {
                        return Err(ModelError::UnknownColumn { table: self.name.clone(), column: wanted.clone() });
                    }
                }
                self.columns.retain(|c| columns.iter().any(|w| w.eq_ignore_ascii_case(&c.name)));
                Ok(())
            }
            TransformStep::RenameColumn { from, to } => {
                let col = self
                    .columns
                    .iter_mut()
                    .find(|c| c.name.eq_ignore_ascii_case(from))
                    .ok_or_else(|| ModelError::UnknownColumn { table: self.name.clone(), column: from.clone() })?;
                col.name = to.trim().to_string();
                Ok(())
            }
            TransformStep::AddCalculatedColumn { name, formula } => {
                let nrows = self.row_count();
                let mut vals = Vec::with_capacity(nrows);
                for r in 0..nrows {
                    let v = eval_row_formula(self, r, formula)?;
                    vals.push(Scalar::Float(v));
                }
                self.columns.retain(|c| !c.name.eq_ignore_ascii_case(name));
                self.columns.push(Column { name: name.trim().to_string(), values: vals });
                Ok(())
            }
            TransformStep::FillNull { column, replacement } => {
                let rep = Scalar::parse_auto(replacement);
                let col = self
                    .columns
                    .iter_mut()
                    .find(|c| c.name.eq_ignore_ascii_case(column))
                    .ok_or_else(|| ModelError::UnknownColumn { table: self.name.clone(), column: column.clone() })?;
                for cell in &mut col.values {
                    if matches!(cell, Scalar::Null) {
                        *cell = rep.clone();
                    }
                }
                Ok(())
            }
        }
    }
}

/// Evaluate a row-context arithmetic expression over a single row `row_idx` of `table`.
/// Supports `table[col]`, `[col]`, numeric literals, and `+`, `-`, `*`, `/`.
pub fn eval_row_formula(table: &Table, row_idx: usize, formula: &str) -> Result<f64, ModelError> {
    let s = formula.trim();
    if s.is_empty() {
        return Err(ModelError::MeasureSyntax("empty row formula".into()));
    }
    if let Some((lhs, op, rhs)) = split_row_binop(s) {
        let l = eval_row_formula(table, row_idx, lhs)?;
        let r = eval_row_formula(table, row_idx, rhs)?;
        return Ok(match op {
            '+' => l + r,
            '-' => l - r,
            '*' => l * r,
            '/' => {
                if r.abs() < 1e-12 {
                    0.0
                } else {
                    l / r
                }
            }
            _ => 0.0,
        });
    }
    if s.starts_with('(') && s.ends_with(')') {
        if let Some(inner) = s.get(1..s.len().saturating_sub(1)) {
            return eval_row_formula(table, row_idx, inner);
        }
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(n);
    }
    let col_name = if s.starts_with('[') && s.ends_with(']') {
        s.get(1..s.len().saturating_sub(1)).unwrap_or("").trim().to_string()
    } else {
        let (_t, c) = parse_table_col(s)?;
        c
    };
    let col = table.column(&col_name).ok_or_else(|| ModelError::UnknownColumn { table: table.name.clone(), column: col_name })?;
    Ok(col.values.get(row_idx).and_then(Scalar::as_f64).unwrap_or(0.0))
}

fn split_row_binop(s: &str) -> Option<(&str, char, &str)> {
    let bytes = s.as_bytes();
    let mut depth_paren = 0usize;
    let mut depth_brack = 0usize;
    let mut add_sub: Option<(usize, char)> = None;
    let mut mul_div: Option<(usize, char)> = None;
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'(' => depth_paren = depth_paren.saturating_add(1),
            b')' => depth_paren = depth_paren.saturating_sub(1),
            b'[' => depth_brack = depth_brack.saturating_add(1),
            b']' => depth_brack = depth_brack.saturating_sub(1),
            b'+' if depth_paren == 0 && depth_brack == 0 && i > 0 => add_sub = Some((i, '+')),
            b'-' if depth_paren == 0 && depth_brack == 0 && i > 0 => add_sub = Some((i, '-')),
            b'*' if depth_paren == 0 && depth_brack == 0 && i > 0 => mul_div = Some((i, '*')),
            b'/' if depth_paren == 0 && depth_brack == 0 && i > 0 => mul_div = Some((i, '/')),
            _ => {}
        }
    }
    let (idx, op) = add_sub.or(mul_div)?;
    let lhs = s.get(..idx)?.trim();
    let rhs = s.get(idx + 1..)?.trim();
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    Some((lhs, op, rhs))
}

/// Translate a sequence of `TransformStep`s into a folded SQL `SELECT` statement.
pub fn fold_to_sql(table_name: &str, steps: &[TransformStep]) -> String {
    let mut projections: Vec<String> = Vec::new();
    let mut renames: Vec<(String, String)> = Vec::new();
    let mut calcs: Vec<(String, String)> = Vec::new();
    let mut coalesces: Vec<(String, String)> = Vec::new();
    let mut predicates: Vec<String> = Vec::new();

    for step in steps {
        match step {
            TransformStep::SelectColumns { columns } => {
                projections = columns.clone();
            }
            TransformStep::RenameColumn { from, to } => {
                renames.push((from.clone(), to.clone()));
            }
            TransformStep::AddCalculatedColumn { name, formula } => {
                let sql_expr = formula_to_sql(table_name, formula);
                calcs.push((name.clone(), sql_expr));
            }
            TransformStep::FillNull { column, replacement } => {
                coalesces.push((column.clone(), sql_literal(replacement)));
            }
            TransformStep::FilterRows { column, op, value } => {
                let rhs = if *op == FilterOp::Contains { format!("'%{}%'", value.replace('\'', "''")) } else { sql_literal(value) };
                predicates.push(format!("\"{}\" {} {}", column, op.sql_op(), rhs));
            }
        }
    }

    let mut select_items: Vec<String> = if projections.is_empty() {
        vec!["*".to_string()]
    } else {
        projections
            .iter()
            .map(|c| {
                let expr = coalesces
                    .iter()
                    .find(|(col, _)| col.eq_ignore_ascii_case(c))
                    .map_or_else(|| format!("\"{c}\""), |(_, rep)| format!("COALESCE(\"{c}\", {rep})"));
                if let Some((_, alias)) = renames.iter().find(|(from, _)| from.eq_ignore_ascii_case(c)) {
                    format!("{expr} AS \"{alias}\"")
                } else if coalesces.iter().any(|(col, _)| col.eq_ignore_ascii_case(c)) {
                    format!("{expr} AS \"{c}\"")
                } else {
                    expr
                }
            })
            .collect()
    };

    for (name, expr) in &calcs {
        select_items.push(format!("({expr}) AS \"{name}\""));
    }

    let mut sql = format!("SELECT {} FROM \"{}\"", select_items.join(", "), table_name);
    if !predicates.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&predicates.join(" AND "));
    }
    sql
}

fn formula_to_sql(table_name: &str, formula: &str) -> String {
    let prefix = format!("{table_name}[");
    let mut out = formula.replace(&prefix, "\"");
    out = out.replace(['[', ']'], "\"");
    out
}

fn sql_literal(raw: &str) -> String {
    let t = raw.trim();
    if t.parse::<i64>().is_ok() || t.parse::<f64>().is_ok() {
        t.to_string()
    } else {
        format!("'{}'", t.replace('\'', "''"))
    }
}
