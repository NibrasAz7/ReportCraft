//! `rc-model` — ReportCraft's columnar store, star-schema relationships,
//! filter-context measure engine, and `.reportcraft` project format.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub mod tmdl;
pub mod transform;

pub use transform::{eval_row_formula, fold_to_sql, FilterOp, TransformStep};

pub const MAX_ROWS: usize = 500_000;
pub const MAX_COLS: usize = 1_024;
pub const MAX_EXPR_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    EmptyTable,
    RowLimitExceeded(usize),
    ColumnLimitExceeded(usize),
    UnknownTable(String),
    UnknownColumn { table: String, column: String },
    UnknownMeasure(String),
    InvalidRelationship(String),
    MeasureSyntax(String),
    MeasureDepthExceeded,
    Serialization(String),
    Io(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::EmptyTable => write!(f, "table must contain a header row"),
            ModelError::RowLimitExceeded(n) => write!(f, "row count {n} exceeds limit {MAX_ROWS}"),
            ModelError::ColumnLimitExceeded(n) => write!(f, "column count {n} exceeds limit {MAX_COLS}"),
            ModelError::UnknownTable(t) => write!(f, "unknown table '{t}'"),
            ModelError::UnknownColumn { table, column } => write!(f, "unknown column '{table}[{column}]'"),
            ModelError::UnknownMeasure(m) => write!(f, "unknown measure '[{m}]'"),
            ModelError::InvalidRelationship(msg) => write!(f, "invalid relationship: {msg}"),
            ModelError::MeasureSyntax(msg) => write!(f, "measure syntax error: {msg}"),
            ModelError::MeasureDepthExceeded => write!(f, "measure evaluation exceeded maximum recursion depth {MAX_EXPR_DEPTH}"),
            ModelError::Serialization(msg) => write!(f, "serialization error: {msg}"),
            ModelError::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for ModelError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl Scalar {
    pub fn parse_auto(raw: &str) -> Self {
        let t = raw.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("null") || t.eq_ignore_ascii_case("na") {
            return Scalar::Null;
        }
        if t.eq_ignore_ascii_case("true") {
            return Scalar::Bool(true);
        }
        if t.eq_ignore_ascii_case("false") {
            return Scalar::Bool(false);
        }
        if let Ok(i) = t.parse::<i64>() {
            return Scalar::Int(i);
        }
        if let Ok(v) = t.parse::<f64>() {
            if v.is_finite() {
                return Scalar::Float(v);
            }
        }
        Scalar::Text(t.to_string())
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Scalar::Int(i) => Some(*i as f64),
            Scalar::Float(v) => Some(*v),
            Scalar::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Scalar::Null | Scalar::Text(_) => None,
        }
    }

    pub fn display_key(&self) -> String {
        match self {
            Scalar::Null => "(Blank)".to_string(),
            Scalar::Bool(b) => b.to_string(),
            Scalar::Int(i) => i.to_string(),
            Scalar::Float(v) => {
                if (v - v.round()).abs() < 1e-9 {
                    format!("{:.0}", v)
                } else {
                    format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            Scalar::Text(s) => s.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub values: Vec<Scalar>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
}

impl Table {
    pub fn row_count(&self) -> usize {
        self.columns.first().map_or(0, |c| c.values.len())
    }

    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn from_delimited(name: impl Into<String>, text: &str, delimiter: char) -> Result<Self, ModelError> {
        let mut lines = text.lines().filter(|l| !l.trim().is_empty());
        let Some(header_line) = lines.next() else {
            return Err(ModelError::EmptyTable);
        };
        let headers = split_delimited_line(header_line, delimiter);
        if headers.is_empty() {
            return Err(ModelError::EmptyTable);
        }
        if headers.len() > MAX_COLS {
            return Err(ModelError::ColumnLimitExceeded(headers.len()));
        }
        let mut columns: Vec<Column> = headers.into_iter().map(|h| Column { name: h.trim().to_string(), values: Vec::new() }).collect();

        let mut row_count = 0usize;
        for line in lines {
            row_count = row_count.saturating_add(1);
            if row_count > MAX_ROWS {
                return Err(ModelError::RowLimitExceeded(row_count));
            }
            let cells = split_delimited_line(line, delimiter);
            for (idx, col) in columns.iter_mut().enumerate() {
                let cell = cells.get(idx).map_or("", String::as_str);
                col.values.push(Scalar::parse_auto(cell));
            }
        }
        Ok(Table { name: name.into(), columns })
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        let headers: Vec<String> = self.columns.iter().map(|c| escape_csv(&c.name)).collect();
        out.push_str(&headers.join(","));
        out.push('\n');
        let nrows = self.row_count();
        for r in 0..nrows {
            let row: Vec<String> = self
                .columns
                .iter()
                .map(|c| match c.values.get(r).unwrap_or(&Scalar::Null) {
                    Scalar::Null => String::new(),
                    other => escape_csv(&other.display_key()),
                })
                .collect();
            out.push_str(&row.join(","));
            out.push('\n');
        }
        out
    }
}

fn escape_csv(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

fn split_delimited_line(line: &str, delimiter: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if in_quotes && chars.peek() == Some(&'"') {
                cur.push('"');
                let _ = chars.next();
            } else {
                in_quotes = !in_quotes;
            }
        } else if c == delimiter && !in_quotes {
            fields.push(cur.trim().to_string());
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    fields.push(cur.trim().to_string());
    fields
}

/// Star-schema relationship: `from_table[from_column]` (many-side / fact)
/// references `to_table[to_column]` (one-side / dimension).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relationship {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

/// Named DAX-class measure expression (`SUM(sales[revenue])`, `CALCULATE(...)`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measure {
    pub name: String,
    pub expression: String,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualKind {
    Bar,
    Line,
    Area,
    Scatter,
    Pie,
    Treemap,
    Gauge,
    KpiCard,
    Table,
    Matrix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisualSpec {
    pub id: String,
    pub title: String,
    pub kind: VisualKind,
    pub group_by: Option<String>,
    pub measures: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterPredicate {
    pub table: String,
    pub column: String,
    pub allowed: Vec<String>,
}

/// Row-Level Security (RLS) role restricting table visibility per identity/role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RlsRole {
    pub name: String,
    pub filters: Vec<FilterPredicate>,
}

/// Query result for a visual or tabular query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub group_column: Option<String>,
    pub measure_names: Vec<String>,
    pub rows: Vec<QueryRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryRow {
    pub group_key: String,
    pub values: Vec<f64>,
}

/// Text-native `.reportcraft` semantic model + report specification.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReportProject {
    pub version: u32,
    pub title: String,
    pub tables: BTreeMap<String, Table>,
    pub relationships: Vec<Relationship>,
    pub measures: BTreeMap<String, Measure>,
    pub visuals: Vec<VisualSpec>,
    pub slicers: Vec<FilterPredicate>,
    #[serde(default)]
    pub rls_roles: Vec<RlsRole>,
}

impl ReportProject {
    pub fn new(title: impl Into<String>) -> Self {
        ReportProject {
            version: 1,
            title: title.into(),
            tables: BTreeMap::new(),
            relationships: Vec::new(),
            measures: BTreeMap::new(),
            visuals: Vec::new(),
            slicers: Vec::new(),
            rls_roles: Vec::new(),
        }
    }

    /// Seeded Northwind star-schema BI project (`regions`, `products`, `orders`,
    /// 2 star-schema relationships, 5 DAX-class measures, and 4 visuals).
    pub fn sample_northwind() -> Self {
        let mut proj = ReportProject::new("Northwind Global Revenue & Margin");
        if let Ok(t) = Table::from_delimited("regions", "region_id,region_name,director\nR1,EMEA,Alice\nR2,AMER,Bob\nR3,APAC,Carol\n", ',') {
            proj.add_table(t);
        }
        if let Ok(t) = Table::from_delimited(
            "products",
            "product_id,product_name,category\nP1,Cloud Enterprise,Cloud\nP2,Analytics Pro,Cloud\nP3,Desktop Workstation,On-Prem\n",
            ',',
        ) {
            proj.add_table(t);
        }
        if let Ok(t) = Table::from_delimited(
            "orders",
            "order_id,region_id,product_id,quarter,units,revenue,cost\n\
             1,R1,P1,Q1,12,24000,9000\n\
             2,R1,P3,Q1,8,12000,7200\n\
             3,R2,P1,Q1,20,40000,14000\n\
             4,R2,P2,Q2,15,22500,8000\n\
             5,R3,P1,Q2,10,20000,7500\n\
             6,R1,P2,Q3,18,27000,9500\n\
             7,R2,P3,Q3,14,21000,12600\n\
             8,R3,P2,Q4,16,24000,8400\n\
             9,R2,P1,Q4,25,50000,17500\n",
            ',',
        ) {
            proj.add_table(t);
        }
        let _ = proj.add_relationship(Relationship {
            from_table: "orders".into(),
            from_column: "region_id".into(),
            to_table: "regions".into(),
            to_column: "region_id".into(),
        });
        let _ = proj.add_relationship(Relationship {
            from_table: "orders".into(),
            from_column: "product_id".into(),
            to_table: "products".into(),
            to_column: "product_id".into(),
        });
        for (name, expression, fmt) in [
            ("Total Revenue", "SUM(orders[revenue])", Some("currency")),
            ("Total Cost", "SUM(orders[cost])", Some("currency")),
            ("Gross Profit", "[Total Revenue] - [Total Cost]", Some("currency")),
            ("Gross Margin %", "DIVIDE([Gross Profit], [Total Revenue], 0)", Some("percent")),
            ("Cloud Revenue", "CALCULATE([Total Revenue], products[category] = \"Cloud\")", Some("currency")),
        ] {
            let _ = proj.add_measure(Measure { name: name.into(), expression: expression.into(), format: fmt.map(String::from) });
        }
        proj.visuals = vec![
            VisualSpec {
                id: "kpi_rev".into(),
                title: "Total Revenue".into(),
                kind: VisualKind::KpiCard,
                group_by: None,
                measures: vec!["Total Revenue".into()],
            },
            VisualSpec {
                id: "bar_region".into(),
                title: "Revenue by Region".into(),
                kind: VisualKind::Bar,
                group_by: Some("regions[region_name]".into()),
                measures: vec!["Total Revenue".into()],
            },
            VisualSpec {
                id: "line_qtr".into(),
                title: "Quarterly Revenue Trend".into(),
                kind: VisualKind::Line,
                group_by: Some("orders[quarter]".into()),
                measures: vec!["Total Revenue".into()],
            },
            VisualSpec {
                id: "tbl_perf".into(),
                title: "Regional Profit & Margin".into(),
                kind: VisualKind::Table,
                group_by: Some("regions[region_name]".into()),
                measures: vec!["Total Revenue".into(), "Gross Profit".into(), "Gross Margin %".into()],
            },
        ];
        proj
    }

    pub fn add_table(&mut self, table: Table) {
        self.tables.insert(table.name.clone(), table);
    }

    pub fn add_relationship(&mut self, rel: Relationship) -> Result<(), ModelError> {
        let from_t = self.tables.get(&rel.from_table).ok_or_else(|| ModelError::UnknownTable(rel.from_table.clone()))?;
        if from_t.column(&rel.from_column).is_none() {
            return Err(ModelError::UnknownColumn { table: rel.from_table, column: rel.from_column });
        }
        let to_t = self.tables.get(&rel.to_table).ok_or_else(|| ModelError::UnknownTable(rel.to_table.clone()))?;
        if to_t.column(&rel.to_column).is_none() {
            return Err(ModelError::UnknownColumn { table: rel.to_table, column: rel.to_column });
        }
        if rel.from_table == rel.to_table {
            return Err(ModelError::InvalidRelationship("self-relationships are not supported".into()));
        }
        self.relationships.retain(|r| !(r.from_table == rel.from_table && r.to_table == rel.to_table));
        self.relationships.push(rel);
        Ok(())
    }

    pub fn add_measure(&mut self, measure: Measure) -> Result<(), ModelError> {
        let _ = parse_measure_expr(&measure.expression)?;
        self.measures.insert(measure.name.clone(), measure);
        Ok(())
    }

    pub fn add_rls_role(&mut self, role: RlsRole) {
        self.rls_roles.retain(|r| !r.name.eq_ignore_ascii_case(&role.name));
        self.rls_roles.push(role);
    }

    pub fn evaluate_measure_as_role(&self, name: &str, role_name: &str) -> Result<f64, ModelError> {
        let role = self
            .rls_roles
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(role_name))
            .ok_or_else(|| ModelError::MeasureSyntax(format!("unknown RLS role '{role_name}'")))?;
        self.evaluate_measure(name, &role.filters)
    }

    pub fn query_as_role(&self, group_by: Option<&str>, measure_names: &[String], role_name: &str) -> Result<QueryResult, ModelError> {
        let role = self
            .rls_roles
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(role_name))
            .ok_or_else(|| ModelError::MeasureSyntax(format!("unknown RLS role '{role_name}'")))?;
        self.query(group_by, measure_names, &role.filters)
    }

    pub fn set_slicer(&mut self, table: &str, column: &str, allowed: Vec<String>) -> Result<(), ModelError> {
        let t = self.tables.get(table).ok_or_else(|| ModelError::UnknownTable(table.to_string()))?;
        if t.column(column).is_none() {
            return Err(ModelError::UnknownColumn { table: table.to_string(), column: column.to_string() });
        }
        self.slicers.retain(|s| !(s.table == table && s.column.eq_ignore_ascii_case(column)));
        if !allowed.is_empty() {
            self.slicers.push(FilterPredicate { table: table.to_string(), column: column.to_string(), allowed });
        }
        Ok(())
    }

    pub fn evaluate_measure(&self, name: &str, extra_filters: &[FilterPredicate]) -> Result<f64, ModelError> {
        let mut ctx = self.slicers.clone();
        for f in extra_filters {
            ctx.retain(|existing| !(existing.table == f.table && existing.column.eq_ignore_ascii_case(&f.column)));
            ctx.push(f.clone());
        }
        self.eval_named_measure(name, &ctx, 0)
    }

    pub fn query(&self, group_by: Option<&str>, measure_names: &[String], extra_filters: &[FilterPredicate]) -> Result<QueryResult, ModelError> {
        if measure_names.is_empty() {
            return Err(ModelError::MeasureSyntax("query requires at least one measure".into()));
        }
        let Some(group_spec) = group_by else {
            let mut vals = Vec::with_capacity(measure_names.len());
            for m in measure_names {
                vals.push(self.evaluate_measure(m, extra_filters)?);
            }
            return Ok(QueryResult {
                group_column: None,
                measure_names: measure_names.to_vec(),
                rows: vec![QueryRow { group_key: "Total".to_string(), values: vals }],
            });
        };

        let (g_table, g_col) = parse_table_col(group_spec)?;
        let table = self.tables.get(&g_table).ok_or_else(|| ModelError::UnknownTable(g_table.clone()))?;
        let col = table.column(&g_col).ok_or_else(|| ModelError::UnknownColumn { table: g_table.clone(), column: g_col.clone() })?;

        let mut base_ctx = self.slicers.clone();
        for f in extra_filters {
            base_ctx.retain(|existing| !(existing.table == f.table && existing.column.eq_ignore_ascii_case(&f.column)));
            base_ctx.push(f.clone());
        }
        let visible_rows = self.matching_rows(&g_table, &base_ctx)?;
        let mut distinct_keys = BTreeSet::new();
        for r in visible_rows {
            if let Some(val) = col.values.get(r) {
                distinct_keys.insert(val.display_key());
            }
        }

        let mut rows = Vec::with_capacity(distinct_keys.len());
        for key in distinct_keys {
            let mut row_filters = extra_filters.to_vec();
            row_filters.push(FilterPredicate { table: g_table.clone(), column: g_col.clone(), allowed: vec![key.clone()] });
            let mut values = Vec::with_capacity(measure_names.len());
            for m in measure_names {
                values.push(self.evaluate_measure(m, &row_filters)?);
            }
            rows.push(QueryRow { group_key: key, values });
        }

        Ok(QueryResult { group_column: Some(format!("{g_table}[{g_col}]")), measure_names: measure_names.to_vec(), rows })
    }

    pub fn to_json_pretty(&self) -> Result<String, ModelError> {
        serde_json::to_string_pretty(self).map_err(|e| ModelError::Serialization(e.to_string()))
    }

    pub fn from_json(text: &str) -> Result<Self, ModelError> {
        serde_json::from_str(text).map_err(|e| ModelError::Serialization(e.to_string()))
    }

    fn eval_named_measure(&self, name: &str, filters: &[FilterPredicate], depth: usize) -> Result<f64, ModelError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(ModelError::MeasureDepthExceeded);
        }
        let m = self.measures.get(name).ok_or_else(|| ModelError::UnknownMeasure(name.to_string()))?;
        let expr = parse_measure_expr(&m.expression)?;
        self.eval_expr(&expr, filters, depth + 1)
    }

    fn eval_expr(&self, expr: &MeasureExpr, filters: &[FilterPredicate], depth: usize) -> Result<f64, ModelError> {
        if depth > MAX_EXPR_DEPTH {
            return Err(ModelError::MeasureDepthExceeded);
        }
        match expr {
            MeasureExpr::Number(n) => Ok(*n),
            MeasureExpr::Ref(name) => self.eval_named_measure(name, filters, depth + 1),
            MeasureExpr::Agg { func, table, column } => {
                let t = self.tables.get(table).ok_or_else(|| ModelError::UnknownTable(table.clone()))?;
                let col = t.column(column).ok_or_else(|| ModelError::UnknownColumn { table: table.clone(), column: column.clone() })?;
                let rows = self.matching_rows(table, filters)?;
                match func {
                    AggFunc::Count => Ok(rows.len() as f64),
                    AggFunc::DistinctCount => {
                        let mut set = BTreeSet::new();
                        for r in rows {
                            if let Some(v) = col.values.get(r) {
                                if !matches!(v, Scalar::Null) {
                                    set.insert(v.display_key());
                                }
                            }
                        }
                        Ok(set.len() as f64)
                    }
                    AggFunc::Sum => {
                        let mut sum = 0.0;
                        for r in rows {
                            if let Some(v) = col.values.get(r).and_then(Scalar::as_f64) {
                                sum += v;
                            }
                        }
                        Ok(sum)
                    }
                    AggFunc::Average => {
                        let mut sum = 0.0;
                        let mut count = 0usize;
                        for r in rows {
                            if let Some(v) = col.values.get(r).and_then(Scalar::as_f64) {
                                sum += v;
                                count = count.saturating_add(1);
                            }
                        }
                        Ok(if count == 0 { 0.0 } else { sum / count as f64 })
                    }
                    AggFunc::Min => {
                        let mut best: Option<f64> = None;
                        for r in rows {
                            if let Some(v) = col.values.get(r).and_then(Scalar::as_f64) {
                                best = Some(best.map_or(v, |b| b.min(v)));
                            }
                        }
                        Ok(best.unwrap_or(0.0))
                    }
                    AggFunc::Max => {
                        let mut best: Option<f64> = None;
                        for r in rows {
                            if let Some(v) = col.values.get(r).and_then(Scalar::as_f64) {
                                best = Some(best.map_or(v, |b| b.max(v)));
                            }
                        }
                        Ok(best.unwrap_or(0.0))
                    }
                }
            }
            MeasureExpr::Divide { num, den, alt } => {
                let n = self.eval_expr(num, filters, depth + 1)?;
                let d = self.eval_expr(den, filters, depth + 1)?;
                if d.abs() < 1e-12 {
                    Ok(*alt)
                } else {
                    Ok(n / d)
                }
            }
            MeasureExpr::Binary { op, lhs, rhs } => {
                let l = self.eval_expr(lhs, filters, depth + 1)?;
                let r = self.eval_expr(rhs, filters, depth + 1)?;
                Ok(match op {
                    BinOp::Add => l + r,
                    BinOp::Sub => l - r,
                    BinOp::Mul => l * r,
                    BinOp::Div => {
                        if r.abs() < 1e-12 {
                            0.0
                        } else {
                            l / r
                        }
                    }
                })
            }
            MeasureExpr::Calculate { inner, overrides } => {
                let mut next_filters = filters.to_vec();
                for ov in overrides {
                    next_filters.retain(|f| !(f.table == ov.table && f.column.eq_ignore_ascii_case(&ov.column)));
                    if !ov.allowed.is_empty() {
                        next_filters.push(ov.clone());
                    }
                }
                self.eval_expr(inner, &next_filters, depth + 1)
            }
            MeasureExpr::Iter { func, table, row_formula } => {
                let t = self.tables.get(table).ok_or_else(|| ModelError::UnknownTable(table.clone()))?;
                let rows = self.matching_rows(table, filters)?;
                if rows.is_empty() {
                    return Ok(0.0);
                }
                let mut vals = Vec::with_capacity(rows.len());
                for r in rows {
                    vals.push(eval_row_formula(t, r, row_formula)?);
                }
                Ok(match func {
                    AggFunc::Sum => vals.iter().sum(),
                    AggFunc::Average => vals.iter().sum::<f64>() / (vals.len() as f64),
                    AggFunc::Min => vals.into_iter().fold(f64::INFINITY, f64::min),
                    AggFunc::Max => vals.into_iter().fold(f64::NEG_INFINITY, f64::max),
                    AggFunc::Count | AggFunc::DistinctCount => vals.len() as f64,
                })
            }
            MeasureExpr::TotalYtd { inner, table, column, cutoff } => {
                let t = self.tables.get(table).ok_or_else(|| ModelError::UnknownTable(table.clone()))?;
                let col = t.column(column).ok_or_else(|| ModelError::UnknownColumn { table: table.clone(), column: column.clone() })?;
                let mut allowed_periods = BTreeSet::new();
                for v in &col.values {
                    let k = v.display_key();
                    if k.as_str() <= cutoff.as_str() {
                        allowed_periods.insert(k);
                    }
                }
                let mut next_filters = filters.to_vec();
                next_filters.retain(|f| !(f.table == *table && f.column.eq_ignore_ascii_case(column)));
                next_filters.push(FilterPredicate { table: table.clone(), column: column.clone(), allowed: allowed_periods.into_iter().collect() });
                self.eval_expr(inner, &next_filters, depth + 1)
            }
        }
    }

    /// Resolve the row indices in `table_name` that satisfy both direct filters
    /// on `table_name` and filters propagated from related dimension tables.
    fn matching_rows(&self, table_name: &str, filters: &[FilterPredicate]) -> Result<Vec<usize>, ModelError> {
        let table = self.tables.get(table_name).ok_or_else(|| ModelError::UnknownTable(table_name.to_string()))?;
        let nrows = table.row_count();
        let mut keep = vec![true; nrows];

        // 1. Direct predicates on `table_name`.
        for pred in filters.iter().filter(|f| f.table == table_name && !f.allowed.is_empty()) {
            let col =
                table.column(&pred.column).ok_or_else(|| ModelError::UnknownColumn { table: table_name.to_string(), column: pred.column.clone() })?;
            let allowed_set: BTreeSet<String> = pred.allowed.iter().cloned().collect();
            for (r, flag) in keep.iter_mut().enumerate() {
                if *flag {
                    let key = col.values.get(r).map_or_else(String::new, Scalar::display_key);
                    if !allowed_set.contains(&key) {
                        *flag = false;
                    }
                }
            }
        }

        // 2. Star-schema propagation: for each relationship `table_name[fk] -> dim[pk]`,
        // if `dim` has active filters, restrict `table_name` rows to matching `pk` values.
        for rel in self.relationships.iter().filter(|r| r.from_table == table_name) {
            let dim_has_filters = filters.iter().any(|f| f.table == rel.to_table && !f.allowed.is_empty());
            if !dim_has_filters {
                continue;
            }
            let dim_table = self.tables.get(&rel.to_table).ok_or_else(|| ModelError::UnknownTable(rel.to_table.clone()))?;
            let dim_pk = dim_table
                .column(&rel.to_column)
                .ok_or_else(|| ModelError::UnknownColumn { table: rel.to_table.clone(), column: rel.to_column.clone() })?;
            let dim_rows = self.matching_rows_direct(&rel.to_table, filters)?;
            let mut valid_keys = BTreeSet::new();
            for r in dim_rows {
                if let Some(v) = dim_pk.values.get(r) {
                    valid_keys.insert(v.display_key());
                }
            }
            let fk_col = table
                .column(&rel.from_column)
                .ok_or_else(|| ModelError::UnknownColumn { table: table_name.to_string(), column: rel.from_column.clone() })?;
            for (r, flag) in keep.iter_mut().enumerate() {
                if *flag {
                    let key = fk_col.values.get(r).map_or_else(String::new, Scalar::display_key);
                    if !valid_keys.contains(&key) {
                        *flag = false;
                    }
                }
            }
        }

        Ok(keep.into_iter().enumerate().filter_map(|(i, ok)| ok.then_some(i)).collect())
    }

    fn matching_rows_direct(&self, table_name: &str, filters: &[FilterPredicate]) -> Result<Vec<usize>, ModelError> {
        let table = self.tables.get(table_name).ok_or_else(|| ModelError::UnknownTable(table_name.to_string()))?;
        let nrows = table.row_count();
        let mut keep = vec![true; nrows];
        for pred in filters.iter().filter(|f| f.table == table_name && !f.allowed.is_empty()) {
            let col =
                table.column(&pred.column).ok_or_else(|| ModelError::UnknownColumn { table: table_name.to_string(), column: pred.column.clone() })?;
            let allowed_set: BTreeSet<String> = pred.allowed.iter().cloned().collect();
            for (r, flag) in keep.iter_mut().enumerate() {
                if *flag {
                    let key = col.values.get(r).map_or_else(String::new, Scalar::display_key);
                    if !allowed_set.contains(&key) {
                        *flag = false;
                    }
                }
            }
        }
        Ok(keep.into_iter().enumerate().filter_map(|(i, ok)| ok.then_some(i)).collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AggFunc {
    Sum,
    Average,
    Min,
    Max,
    Count,
    DistinctCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Clone, PartialEq)]
enum MeasureExpr {
    Number(f64),
    Ref(String),
    Agg { func: AggFunc, table: String, column: String },
    Iter { func: AggFunc, table: String, row_formula: String },
    TotalYtd { inner: Box<MeasureExpr>, table: String, column: String, cutoff: String },
    Divide { num: Box<MeasureExpr>, den: Box<MeasureExpr>, alt: f64 },
    Binary { op: BinOp, lhs: Box<MeasureExpr>, rhs: Box<MeasureExpr> },
    Calculate { inner: Box<MeasureExpr>, overrides: Vec<FilterPredicate> },
}

pub fn parse_table_col(spec: &str) -> Result<(String, String), ModelError> {
    let s = spec.trim();
    let Some(open) = s.find('[') else {
        return Err(ModelError::MeasureSyntax(format!("expected table[column], got '{s}'")));
    };
    let Some(close) = s.rfind(']') else {
        return Err(ModelError::MeasureSyntax(format!("missing closing ']' in '{s}'")));
    };
    if close <= open + 1 {
        return Err(ModelError::MeasureSyntax(format!("empty column in '{s}'")));
    }
    let table = s.get(..open).unwrap_or("").trim().trim_matches('\'');
    let col = s.get(open + 1..close).unwrap_or("").trim();
    if table.is_empty() || col.is_empty() {
        return Err(ModelError::MeasureSyntax(format!("invalid table[column] reference '{s}'")));
    }
    Ok((table.to_string(), col.to_string()))
}

fn parse_measure_expr(raw: &str) -> Result<MeasureExpr, ModelError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(ModelError::MeasureSyntax("empty measure expression".into()));
    }
    if let Some((lhs, op, rhs)) = split_top_level_binop(s) {
        return Ok(MeasureExpr::Binary { op, lhs: Box::new(parse_measure_expr(lhs)?), rhs: Box::new(parse_measure_expr(rhs)?) });
    }
    if let Ok(v) = s.parse::<f64>() {
        return Ok(MeasureExpr::Number(v));
    }
    if s.starts_with('[') && s.ends_with(']') && !s.get(1..s.len().saturating_sub(1)).unwrap_or("").contains('[') {
        let name = s.get(1..s.len().saturating_sub(1)).unwrap_or("").trim();
        if name.is_empty() {
            return Err(ModelError::MeasureSyntax("empty measure reference []".into()));
        }
        return Ok(MeasureExpr::Ref(name.to_string()));
    }
    let Some(open_paren) = s.find('(') else {
        return Err(ModelError::MeasureSyntax(format!("unrecognized measure expression '{s}'")));
    };
    if !s.ends_with(')') {
        return Err(ModelError::MeasureSyntax(format!("missing closing ')' in '{s}'")));
    }
    let fn_name = s.get(..open_paren).unwrap_or("").trim().to_ascii_uppercase();
    let args_str = s.get(open_paren + 1..s.len().saturating_sub(1)).unwrap_or("").trim();
    let args = split_top_level_args(args_str);

    match fn_name.as_str() {
        "SUM" | "AVERAGE" | "AVG" | "MIN" | "MAX" | "COUNT" | "DISTINCTCOUNT" => {
            if args.len() != 1 {
                return Err(ModelError::MeasureSyntax(format!("{fn_name} expects 1 argument")));
            }
            let (table, column) = parse_table_col(args.first().map_or("", String::as_str))?;
            let func = match fn_name.as_str() {
                "SUM" => AggFunc::Sum,
                "AVERAGE" | "AVG" => AggFunc::Average,
                "MIN" => AggFunc::Min,
                "MAX" => AggFunc::Max,
                "COUNT" => AggFunc::Count,
                _ => AggFunc::DistinctCount,
            };
            Ok(MeasureExpr::Agg { func, table, column })
        }
        "DIVIDE" => {
            if args.len() < 2 || args.len() > 3 {
                return Err(ModelError::MeasureSyntax("DIVIDE expects 2 or 3 arguments".into()));
            }
            let num = parse_measure_expr(args.first().map_or("", String::as_str))?;
            let den = parse_measure_expr(args.get(1).map_or("", String::as_str))?;
            let alt = match args.get(2) {
                Some(a) => a.trim().parse::<f64>().map_err(|_| ModelError::MeasureSyntax(format!("invalid DIVIDE alternate result '{a}'")))?,
                None => 0.0,
            };
            Ok(MeasureExpr::Divide { num: Box::new(num), den: Box::new(den), alt })
        }
        "CALCULATE" => {
            if args.is_empty() {
                return Err(ModelError::MeasureSyntax("CALCULATE expects at least 1 argument".into()));
            }
            let inner = parse_measure_expr(args.first().map_or("", String::as_str))?;
            let mut overrides = Vec::new();
            for filter_arg in args.iter().skip(1) {
                overrides.push(parse_calculate_filter(filter_arg)?);
            }
            Ok(MeasureExpr::Calculate { inner: Box::new(inner), overrides })
        }
        "SUMX" | "AVERAGEX" | "MINX" | "MAXX" | "COUNTX" => {
            if args.len() != 2 {
                return Err(ModelError::MeasureSyntax(format!("{fn_name} expects 2 arguments (table, row_expression)")));
            }
            let table = args.first().map_or("", String::as_str).trim().to_string();
            let row_formula = args.get(1).map_or("", String::as_str).trim().to_string();
            if table.is_empty() || row_formula.is_empty() {
                return Err(ModelError::MeasureSyntax(format!("{fn_name} requires non-empty table and expression")));
            }
            let func = match fn_name.as_str() {
                "SUMX" => AggFunc::Sum,
                "AVERAGEX" => AggFunc::Average,
                "MINX" => AggFunc::Min,
                "MAXX" => AggFunc::Max,
                _ => AggFunc::Count,
            };
            Ok(MeasureExpr::Iter { func, table, row_formula })
        }
        "TOTALYTD" => {
            if args.len() < 2 || args.len() > 3 {
                return Err(ModelError::MeasureSyntax("TOTALYTD expects (measure_expr, table[period_col], [cutoff])".into()));
            }
            let inner = parse_measure_expr(args.first().map_or("", String::as_str))?;
            let (table, column) = parse_table_col(args.get(1).map_or("", String::as_str))?;
            let cutoff = args.get(2).map_or("Q4", String::as_str).trim().trim_matches('"').trim_matches('\'').to_string();
            Ok(MeasureExpr::TotalYtd { inner: Box::new(inner), table, column, cutoff })
        }
        _ => Err(ModelError::MeasureSyntax(format!("unknown function '{fn_name}'"))),
    }
}

fn parse_calculate_filter(arg: &str) -> Result<FilterPredicate, ModelError> {
    let t = arg.trim();
    if let Some(inner) = t.strip_prefix("ALL(").or_else(|| t.strip_prefix("all(")).and_then(|s| s.strip_suffix(')')) {
        let (table, column) = parse_table_col(inner)?;
        return Ok(FilterPredicate { table, column, allowed: Vec::new() });
    }
    let Some(eq_idx) = t.find('=') else {
        return Err(ModelError::MeasureSyntax(format!("expected table[col] = value in CALCULATE filter, got '{t}'")));
    };
    let lhs = t.get(..eq_idx).unwrap_or("").trim();
    let rhs = t.get(eq_idx + 1..).unwrap_or("").trim().trim_matches('"').trim_matches('\'');
    let (table, column) = parse_table_col(lhs)?;
    Ok(FilterPredicate { table, column, allowed: vec![rhs.to_string()] })
}

fn split_top_level_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth_paren = 0usize;
    let mut depth_brack = 0usize;
    let mut in_quotes = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(c);
            }
            '(' if !in_quotes => {
                depth_paren = depth_paren.saturating_add(1);
                cur.push(c);
            }
            ')' if !in_quotes => {
                depth_paren = depth_paren.saturating_sub(1);
                cur.push(c);
            }
            '[' if !in_quotes => {
                depth_brack = depth_brack.saturating_add(1);
                cur.push(c);
            }
            ']' if !in_quotes => {
                depth_brack = depth_brack.saturating_sub(1);
                cur.push(c);
            }
            ',' if !in_quotes && depth_paren == 0 && depth_brack == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn split_top_level_binop(s: &str) -> Option<(&str, BinOp, &str)> {
    let bytes = s.as_bytes();
    let mut depth_paren = 0usize;
    let mut depth_brack = 0usize;
    let mut in_quotes = false;
    let mut add_sub_pos: Option<(usize, BinOp)> = None;
    let mut mul_div_pos: Option<(usize, BinOp)> = None;

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'"' => in_quotes = !in_quotes,
            b'(' if !in_quotes => depth_paren = depth_paren.saturating_add(1),
            b')' if !in_quotes => depth_paren = depth_paren.saturating_sub(1),
            b'[' if !in_quotes => depth_brack = depth_brack.saturating_add(1),
            b']' if !in_quotes => depth_brack = depth_brack.saturating_sub(1),
            b'+' if !in_quotes && depth_paren == 0 && depth_brack == 0 && i > 0 => add_sub_pos = Some((i, BinOp::Add)),
            b'-' if !in_quotes && depth_paren == 0 && depth_brack == 0 && i > 0 => add_sub_pos = Some((i, BinOp::Sub)),
            b'*' if !in_quotes && depth_paren == 0 && depth_brack == 0 && i > 0 => mul_div_pos = Some((i, BinOp::Mul)),
            b'/' if !in_quotes && depth_paren == 0 && depth_brack == 0 && i > 0 => mul_div_pos = Some((i, BinOp::Div)),
            _ => {}
        }
    }
    let (idx, op) = add_sub_pos.or(mul_div_pos)?;
    let lhs = s.get(..idx)?.trim();
    let rhs = s.get(idx + 1..)?.trim();
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    Some((lhs, op, rhs))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_project() -> ReportProject {
        let mut proj = ReportProject::new("Regional Sales");
        let regions = Table::from_delimited("regions", "region_id,region_name,director\nR1,EMEA,Alice\nR2,AMER,Bob\nR3,APAC,Carol\n", ',').unwrap();
        let sales = Table::from_delimited(
            "sales",
            "order_id,region_id,category,revenue,cost\n1,R1,Hardware,1200,800\n2,R1,Software,600,150\n3,R2,Hardware,2000,1400\n4,R3,Software,900,200\n",
            ',',
        )
        .unwrap();
        proj.add_table(regions);
        proj.add_table(sales);
        proj.add_relationship(Relationship {
            from_table: "sales".into(),
            from_column: "region_id".into(),
            to_table: "regions".into(),
            to_column: "region_id".into(),
        })
        .unwrap();
        proj.add_measure(Measure { name: "Total Revenue".into(), expression: "SUM(sales[revenue])".into(), format: Some("currency".into()) })
            .unwrap();
        proj.add_measure(Measure { name: "Total Cost".into(), expression: "SUM(sales[cost])".into(), format: Some("currency".into()) }).unwrap();
        proj.add_measure(Measure {
            name: "Gross Margin %".into(),
            expression: "DIVIDE([Total Revenue] - [Total Cost], [Total Revenue], 0)".into(),
            format: Some("percent".into()),
        })
        .unwrap();
        proj.add_measure(Measure {
            name: "Hardware Revenue".into(),
            expression: "CALCULATE([Total Revenue], sales[category] = \"Hardware\")".into(),
            format: Some("currency".into()),
        })
        .unwrap();
        proj.add_measure(Measure {
            name: "All Regions Revenue".into(),
            expression: "CALCULATE([Total Revenue], ALL(regions[region_name]))".into(),
            format: Some("currency".into()),
        })
        .unwrap();
        proj
    }

    #[test]
    fn star_schema_slicer_and_calculate_context() {
        let mut proj = sample_project();
        assert_eq!(proj.evaluate_measure("Total Revenue", &[]).unwrap(), 4700.0);
        assert_eq!(proj.evaluate_measure("Hardware Revenue", &[]).unwrap(), 3200.0);

        // Slicing dimension table `regions[region_name] = EMEA` propagates to `sales`.
        proj.set_slicer("regions", "region_name", vec!["EMEA".into()]).unwrap();
        assert_eq!(proj.evaluate_measure("Total Revenue", &[]).unwrap(), 1800.0);
        assert_eq!(proj.evaluate_measure("Hardware Revenue", &[]).unwrap(), 1200.0);
        // `ALL(regions[region_name])` inside CALCULATE clears the slicer on region_name.
        assert_eq!(proj.evaluate_measure("All Regions Revenue", &[]).unwrap(), 4700.0);
    }

    #[test]
    fn grouped_visual_query_with_star_schema() {
        let proj = sample_project();
        let res = proj.query(Some("regions[region_name]"), &["Total Revenue".into(), "Gross Margin %".into()], &[]).unwrap();
        assert_eq!(res.rows.len(), 3);
        assert_eq!(res.rows[0].group_key, "AMER");
        assert_eq!(res.rows[0].values[0], 2000.0);
        assert!((res.rows[0].values[1] - 0.30).abs() < 1e-6);
        assert_eq!(res.rows[1].group_key, "APAC");
        assert_eq!(res.rows[1].values[0], 900.0);
        assert_eq!(res.rows[2].group_key, "EMEA");
        assert_eq!(res.rows[2].values[0], 1800.0);
    }

    #[test]
    fn project_json_roundtrip() {
        let proj = sample_project();
        let json = proj.to_json_pretty().unwrap();
        let restored = ReportProject::from_json(&json).unwrap();
        assert_eq!(proj, restored);
    }

    #[test]
    fn sumx_totalytd_rls_tmdl_and_query_folding() {
        let mut proj = ReportProject::sample_northwind();
        proj.add_measure(Measure {
            name: "Net Profit (SUMX)".into(),
            expression: "SUMX(orders, orders[revenue] - orders[cost])".into(),
            format: Some("currency".into()),
        })
        .unwrap();
        proj.add_measure(Measure {
            name: "YTD Q2 Revenue".into(),
            expression: "TOTALYTD([Total Revenue], orders[quarter], \"Q2\")".into(),
            format: Some("currency".into()),
        })
        .unwrap();

        assert_eq!(proj.evaluate_measure("Net Profit (SUMX)", &[]).unwrap(), 146800.0);
        // Q1 (24000+12000+40000=76000) + Q2 (22500+20000=42500) = 118500
        assert_eq!(proj.evaluate_measure("YTD Q2 Revenue", &[]).unwrap(), 118500.0);

        // Row-Level Security role restricting to EMEA
        proj.add_rls_role(RlsRole {
            name: "EMEA_Viewer".into(),
            filters: vec![FilterPredicate { table: "regions".into(), column: "region_name".into(), allowed: vec!["EMEA".into()] }],
        });
        assert_eq!(proj.evaluate_measure_as_role("Total Revenue", "EMEA_Viewer").unwrap(), 63000.0);

        // TMDL roundtrip
        let tmdl = proj.to_tmdl();
        let parsed = ReportProject::from_tmdl("Restored", &tmdl).unwrap();
        assert_eq!(parsed.tables.len(), 3);
        assert_eq!(parsed.relationships.len(), 2);
        assert!(parsed.measures.contains_key("Net Profit (SUMX)"));
        assert_eq!(parsed.rls_roles.len(), 1);

        // Power Query transform + SQL query folding
        let steps = vec![
            TransformStep::SelectColumns { columns: vec!["region_id".into(), "revenue".into(), "cost".into()] },
            TransformStep::AddCalculatedColumn { name: "margin".into(), formula: "orders[revenue] - orders[cost]".into() },
            TransformStep::FilterRows { column: "revenue".into(), op: FilterOp::Ge, value: "20000".into() },
        ];
        let sql = fold_to_sql("orders", &steps);
        assert_eq!(
            sql,
            "SELECT \"region_id\", \"revenue\", \"cost\", (\"revenue\" - \"cost\") AS \"margin\" FROM \"orders\" WHERE \"revenue\" >= 20000"
        );
    }
}
