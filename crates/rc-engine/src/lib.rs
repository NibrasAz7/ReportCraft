//! `rc-engine` — Command registry and guarded `Session` for ReportCraft.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use std::fmt;
use std::path::Path;

use craft_atomic::atomic_write;
use rc_model::{
    fold_to_sql, FilterPredicate, Measure, ModelError, Relationship, ReportProject, RlsRole, Table, TransformStep, VisualKind, VisualSpec,
};
use rc_render::{burst_report_html, render_report_html, render_report_pdf, render_report_svg};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    UnknownCommand(String),
    MissingParam(String),
    InvalidParam(String),
    Model(ModelError),
    Io(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::UnknownCommand(c) => write!(f, "unknown command '{c}'"),
            EngineError::MissingParam(p) => write!(f, "missing required parameter '{p}'"),
            EngineError::InvalidParam(p) => write!(f, "invalid parameter: {p}"),
            EngineError::Model(e) => write!(f, "{e}"),
            EngineError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for EngineError {}

impl From<ModelError> for EngineError {
    fn from(value: ModelError) -> Self {
        EngineError::Model(value)
    }
}

pub struct CommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub params: &'static str,
}

pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec { id: "engine.list", label: "List engine commands", params: "{}" },
    CommandSpec { id: "project.new", label: "Create a new ReportCraft project", params: "{title}" },
    CommandSpec { id: "project.sample", label: "Load the Northwind star-schema BI sample", params: "{}" },
    CommandSpec { id: "project.open", label: "Open a .reportcraft JSON project file", params: "{path}" },
    CommandSpec { id: "project.save", label: "Save the project to a .reportcraft file", params: "{path}" },
    CommandSpec { id: "project.summary", label: "Summarize tables, relationships, measures, visuals", params: "{}" },
    CommandSpec { id: "table.import_csv", label: "Import CSV/TSV text or file into a table", params: "{name, text?, path?, delimiter?}" },
    CommandSpec { id: "table.import_json", label: "Import JSON array of objects into a table", params: "{name, text?, path?}" },
    CommandSpec { id: "table.export_json", label: "Export a table as a JSON array of objects", params: "{name, path?}" },
    CommandSpec { id: "table.transform", label: "Apply Power Query transform steps and fold to SQL", params: "{name, steps: [..]}" },
    CommandSpec { id: "table.fold_sql", label: "Fold Power Query transform steps into a native SQL SELECT", params: "{name, steps: [..]}" },
    CommandSpec { id: "table.list", label: "List tables and row/column counts", params: "{}" },
    CommandSpec { id: "model.relate", label: "Add a star-schema relationship", params: "{from_table, from_column, to_table, to_column}" },
    CommandSpec { id: "model.add_measure", label: "Define or replace a DAX-class measure", params: "{name, expression, format?}" },
    CommandSpec { id: "model.eval_measure", label: "Evaluate a measure in the current slicer context", params: "{name}" },
    CommandSpec { id: "model.add_rls_role", label: "Define a Row-Level Security (RLS) role", params: "{name, table, column, allowed: [..]}" },
    CommandSpec { id: "model.eval_as_role", label: "Evaluate a measure under an RLS role", params: "{measure, role}" },
    CommandSpec { id: "model.query", label: "Run a grouped visual query", params: "{group_by?, measures: [..]}" },
    CommandSpec { id: "slicer.set", label: "Set or clear a slicer on table[column]", params: "{table, column, allowed: [..]}" },
    CommandSpec { id: "visual.add", label: "Add a visual to the report canvas", params: "{id, title, kind, group_by?, measures: [..]}" },
    CommandSpec { id: "interop.export_tmdl", label: "Export semantic model as Power BI TMDL", params: "{path?}" },
    CommandSpec { id: "interop.import_tmdl", label: "Import semantic model from Power BI TMDL", params: "{title?, text?, path?}" },
    CommandSpec { id: "report.export_svg", label: "Render the report to SVG (and optionally write to path)", params: "{path?}" },
    CommandSpec { id: "report.export_html", label: "Render standalone interactive HTML (and optionally write to path)", params: "{path?}" },
    CommandSpec { id: "report.export_pdf", label: "Render multi-page paginated PDF 1.4 report", params: "{path?}" },
    CommandSpec { id: "report.burst_html", label: "Burst parameterized HTML reports across a dimension", params: "{dimension}" },
];

#[derive(Default)]
pub struct Session {
    project: ReportProject,
}

impl Session {
    pub fn new() -> Self {
        Session { project: ReportProject::sample_northwind() }
    }

    pub fn empty() -> Self {
        Session { project: ReportProject::new("Untitled Report") }
    }

    pub fn commands(&self) -> &'static [CommandSpec] {
        COMMANDS
    }

    pub fn project(&self) -> &ReportProject {
        &self.project
    }

    pub fn execute(&mut self, command: &str, params: Value) -> Result<Value, EngineError> {
        match command {
            "engine.list" => {
                let list: Vec<Value> = COMMANDS.iter().map(|c| json!({"id": c.id, "label": c.label, "params": c.params})).collect();
                Ok(json!({"commands": list}))
            }
            "project.new" => {
                let title = req_str(&params, "title")?;
                self.project = ReportProject::new(title);
                Ok(json!({"title": self.project.title}))
            }
            "project.sample" => {
                self.project = ReportProject::sample_northwind();
                Ok(json!({"title": self.project.title, "tables": self.project.tables.len(), "measures": self.project.measures.len()}))
            }
            "project.open" => {
                let path = req_str(&params, "path")?;
                let content = std::fs::read_to_string(&path).map_err(|e| EngineError::Io(e.to_string()))?;
                self.project = ReportProject::from_json(&content)?;
                Ok(json!({"title": self.project.title, "tables": self.project.tables.len()}))
            }
            "project.save" => {
                let path = req_str(&params, "path")?;
                let text = self.project.to_json_pretty()?;
                atomic_write(Path::new(&path), text.as_bytes()).map_err(|e| EngineError::Io(e.to_string()))?;
                Ok(json!({"saved": path, "bytes": text.len()}))
            }
            "project.summary" => {
                let tables: Vec<Value> =
                    self.project.tables.values().map(|t| json!({"name": t.name, "rows": t.row_count(), "columns": t.columns.len()})).collect();
                let measures: Vec<&String> = self.project.measures.keys().collect();
                Ok(json!({
                    "title": self.project.title,
                    "tables": tables,
                    "relationships": self.project.relationships.len(),
                    "measures": measures,
                    "visuals": self.project.visuals.len(),
                    "slicers": self.project.slicers.len(),
                    "rlsRoles": self.project.rls_roles.len(),
                }))
            }
            "table.import_csv" => {
                let name = req_str(&params, "name")?;
                let delim = params.get("delimiter").and_then(Value::as_str).and_then(|s| s.chars().next()).unwrap_or(',');
                let csv_text = if let Some(inline) = params.get("text").and_then(Value::as_str) {
                    inline.to_string()
                } else {
                    let path = req_str(&params, "path")?;
                    std::fs::read_to_string(&path).map_err(|e| EngineError::Io(e.to_string()))?
                };
                let table = Table::from_delimited(&name, &csv_text, delim)?;
                let rows = table.row_count();
                let cols = table.columns.len();
                self.project.add_table(table);
                Ok(json!({"table": name, "rows": rows, "columns": cols}))
            }
            "table.import_json" => {
                let name = req_str(&params, "name")?;
                let json_text = if let Some(inline) = params.get("text").and_then(Value::as_str) {
                    inline.to_string()
                } else {
                    let path = req_str(&params, "path")?;
                    std::fs::read_to_string(&path).map_err(|e| EngineError::Io(e.to_string()))?
                };
                let table = Table::from_json_records(&name, &json_text)?;
                let rows = table.row_count();
                let cols = table.columns.len();
                self.project.add_table(table);
                Ok(json!({"table": name, "rows": rows, "columns": cols}))
            }
            "table.export_json" => {
                let name = req_str(&params, "name")?;
                let table = self.project.tables.get(&name).ok_or_else(|| ModelError::UnknownTable(name.clone()))?;
                let out = table.to_json_records()?;
                if let Some(path) = params.get("path").and_then(Value::as_str) {
                    atomic_write(Path::new(path), out.as_bytes()).map_err(|e| EngineError::Io(e.to_string()))?;
                    Ok(json!({"path": path, "bytes": out.len()}))
                } else {
                    Ok(json!({"json": out}))
                }
            }
            "table.transform" => {
                let name = req_str(&params, "name")?;
                let steps_val = params.get("steps").cloned().ok_or_else(|| EngineError::MissingParam("steps".into()))?;
                let steps: Vec<TransformStep> =
                    serde_json::from_value(steps_val).map_err(|e| EngineError::InvalidParam(format!("invalid transform steps: {e}")))?;
                let sql = fold_to_sql(&name, &steps);
                let table = self.project.tables.get_mut(&name).ok_or_else(|| ModelError::UnknownTable(name.clone()))?;
                table.apply_steps(&steps)?;
                Ok(json!({"table": name, "rows": table.row_count(), "columns": table.columns.len(), "foldedSql": sql}))
            }
            "table.fold_sql" => {
                let name = req_str(&params, "name")?;
                let steps_val = params.get("steps").cloned().ok_or_else(|| EngineError::MissingParam("steps".into()))?;
                let steps: Vec<TransformStep> =
                    serde_json::from_value(steps_val).map_err(|e| EngineError::InvalidParam(format!("invalid transform steps: {e}")))?;
                let sql = fold_to_sql(&name, &steps);
                Ok(json!({"table": name, "foldedSql": sql}))
            }
            "table.list" => {
                let list: Vec<Value> = self
                    .project
                    .tables
                    .values()
                    .map(|t| {
                        let cols: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
                        json!({"name": t.name, "rows": t.row_count(), "columns": cols})
                    })
                    .collect();
                Ok(json!({"tables": list}))
            }
            "model.relate" => {
                let rel = Relationship {
                    from_table: req_str(&params, "from_table")?,
                    from_column: req_str(&params, "from_column")?,
                    to_table: req_str(&params, "to_table")?,
                    to_column: req_str(&params, "to_column")?,
                };
                self.project.add_relationship(rel)?;
                Ok(json!({"relationships": self.project.relationships.len()}))
            }
            "model.add_measure" => {
                let name = req_str(&params, "name")?;
                let expression = req_str(&params, "expression")?;
                let format = params.get("format").and_then(Value::as_str).map(String::from);
                self.project.add_measure(Measure { name: name.clone(), expression, format })?;
                Ok(json!({"measure": name}))
            }
            "model.eval_measure" => {
                let name = req_str(&params, "name")?;
                let value = self.project.evaluate_measure(&name, &[])?;
                Ok(json!({"measure": name, "value": value}))
            }
            "model.add_rls_role" => {
                let name = req_str(&params, "name")?;
                let table = req_str(&params, "table")?;
                let column = req_str(&params, "column")?;
                let allowed = req_str_array(&params, "allowed")?;
                self.project.add_rls_role(RlsRole { name: name.clone(), filters: vec![FilterPredicate { table, column, allowed }] });
                Ok(json!({"role": name, "rlsRoles": self.project.rls_roles.len()}))
            }
            "model.eval_as_role" => {
                let measure = req_str(&params, "measure")?;
                let role = req_str(&params, "role")?;
                let value = self.project.evaluate_measure_as_role(&measure, &role)?;
                Ok(json!({"measure": measure, "role": role, "value": value}))
            }
            "model.query" => {
                let group_by = params.get("group_by").and_then(Value::as_str);
                let measures = req_str_array(&params, "measures")?;
                let result = self.project.query(group_by, &measures, &[])?;
                serde_json::to_value(result).map_err(|e| EngineError::InvalidParam(e.to_string()))
            }
            "slicer.set" => {
                let table = req_str(&params, "table")?;
                let column = req_str(&params, "column")?;
                let allowed = req_str_array(&params, "allowed")?;
                self.project.set_slicer(&table, &column, allowed)?;
                Ok(json!({"slicers": self.project.slicers.len()}))
            }
            "visual.add" => {
                let id = req_str(&params, "id")?;
                let title = req_str(&params, "title")?;
                let kind_str = req_str(&params, "kind")?;
                let kind = match kind_str.as_str() {
                    "bar" => VisualKind::Bar,
                    "line" => VisualKind::Line,
                    "area" => VisualKind::Area,
                    "scatter" => VisualKind::Scatter,
                    "pie" => VisualKind::Pie,
                    "treemap" => VisualKind::Treemap,
                    "gauge" => VisualKind::Gauge,
                    "kpi" | "kpi_card" => VisualKind::KpiCard,
                    "table" => VisualKind::Table,
                    "matrix" => VisualKind::Matrix,
                    other => return Err(EngineError::InvalidParam(format!("unknown visual kind '{other}'"))),
                };
                let group_by = params.get("group_by").and_then(Value::as_str).map(String::from);
                let measures = req_str_array(&params, "measures")?;
                self.project.visuals.retain(|v| v.id != id);
                self.project.visuals.push(VisualSpec { id: id.clone(), title, kind, group_by, measures });
                Ok(json!({"visual": id, "count": self.project.visuals.len()}))
            }
            "interop.export_tmdl" => {
                let tmdl = self.project.to_tmdl();
                if let Some(path) = params.get("path").and_then(Value::as_str) {
                    atomic_write(Path::new(path), tmdl.as_bytes()).map_err(|e| EngineError::Io(e.to_string()))?;
                    Ok(json!({"path": path, "bytes": tmdl.len()}))
                } else {
                    Ok(json!({"tmdl": tmdl}))
                }
            }
            "interop.import_tmdl" => {
                let title = params.get("title").and_then(Value::as_str).unwrap_or("Imported TMDL Model");
                let text = if let Some(inline) = params.get("text").and_then(Value::as_str) {
                    inline.to_string()
                } else {
                    let path = req_str(&params, "path")?;
                    std::fs::read_to_string(&path).map_err(|e| EngineError::Io(e.to_string()))?
                };
                self.project = ReportProject::from_tmdl(title, &text)?;
                Ok(json!({
                    "title": self.project.title,
                    "tables": self.project.tables.len(),
                    "measures": self.project.measures.len(),
                    "relationships": self.project.relationships.len(),
                }))
            }
            "report.export_svg" => {
                let svg = render_report_svg(&self.project)?;
                if let Some(path) = params.get("path").and_then(Value::as_str) {
                    atomic_write(Path::new(path), svg.as_bytes()).map_err(|e| EngineError::Io(e.to_string()))?;
                    Ok(json!({"path": path, "bytes": svg.len()}))
                } else {
                    Ok(json!({"svg": svg}))
                }
            }
            "report.export_html" => {
                let html = render_report_html(&self.project)?;
                if let Some(path) = params.get("path").and_then(Value::as_str) {
                    atomic_write(Path::new(path), html.as_bytes()).map_err(|e| EngineError::Io(e.to_string()))?;
                    Ok(json!({"path": path, "bytes": html.len()}))
                } else {
                    Ok(json!({"html": html}))
                }
            }
            "report.export_pdf" => {
                let pdf = render_report_pdf(&self.project)?;
                if let Some(path) = params.get("path").and_then(Value::as_str) {
                    atomic_write(Path::new(path), &pdf).map_err(|e| EngineError::Io(e.to_string()))?;
                    Ok(json!({"path": path, "bytes": pdf.len()}))
                } else {
                    Ok(json!({"bytes": pdf.len(), "header": "%PDF-1.4"}))
                }
            }
            "report.burst_html" => {
                let dimension = req_str(&params, "dimension")?;
                let bursted = burst_report_html(&self.project, &dimension)?;
                let keys: Vec<String> = bursted.keys().cloned().collect();
                Ok(json!({"dimension": dimension, "slices": keys, "count": keys.len()}))
            }
            other => Err(EngineError::UnknownCommand(other.to_string())),
        }
    }
}

fn req_str(params: &Value, key: &str) -> Result<String, EngineError> {
    params.get(key).and_then(Value::as_str).map(String::from).ok_or_else(|| EngineError::MissingParam(key.to_string()))
}

fn req_str_array(params: &Value, key: &str) -> Result<Vec<String>, EngineError> {
    let arr = params.get(key).and_then(Value::as_array).ok_or_else(|| EngineError::MissingParam(key.to_string()))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let s = item.as_str().ok_or_else(|| EngineError::InvalidParam(format!("'{key}' must contain strings")))?;
        out.push(s.to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_pipeline_builds_and_exports_report() {
        let mut s = Session::new();
        s.execute("project.new", json!({"title": "Northwind Executive Summary"})).unwrap();
        s.execute(
            "table.import_csv",
            json!({
                "name": "orders",
                "text": "region,product,amount\nEMEA,Cloud,1500\nAMER,Cloud,2500\nEMEA,Desktop,800\n"
            }),
        )
        .unwrap();
        s.execute("model.add_measure", json!({"name": "Total Sales", "expression": "SUM(orders[amount])"})).unwrap();
        let eval = s.execute("model.eval_measure", json!({"name": "Total Sales"})).unwrap();
        assert_eq!(eval["value"], 4800.0);

        s.execute(
            "visual.add",
            json!({
                "id": "by_region",
                "title": "Sales by Region",
                "kind": "bar",
                "group_by": "orders[region]",
                "measures": ["Total Sales"]
            }),
        )
        .unwrap();

        let svg = s.execute("report.export_svg", json!({})).unwrap();
        assert!(svg["svg"].as_str().unwrap().contains("Sales by Region"));

        let pdf = s.execute("report.export_pdf", json!({})).unwrap();
        assert_eq!(pdf["header"], "%PDF-1.4");

        let tmdl = s.execute("interop.export_tmdl", json!({})).unwrap();
        assert!(tmdl["tmdl"].as_str().unwrap().contains("measure 'Total Sales'"));
    }
}
