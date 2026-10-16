//! `tmdl` — Power BI `.pbip` Tabular Model Definition Language (TMDL)
//! import and export for `ReportProject`.

use crate::{Column, FilterPredicate, Measure, ModelError, Relationship, ReportProject, RlsRole, Table};

impl ReportProject {
    /// Export the semantic model (tables, columns, measures, relationships,
    /// and RLS roles) in Microsoft Power BI TMDL text format.
    pub fn to_tmdl(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("model '{}'\n\tculture: en-US\n\n", self.title.replace('\'', "''")));

        for table in self.tables.values() {
            out.push_str(&format!("table {}\n", table.name));
            for col in &table.columns {
                out.push_str(&format!("\tcolumn {}\n\t\tsummarizeBy: none\n", col.name));
            }
            out.push('\n');
        }

        for m in self.measures.values() {
            out.push_str(&format!("measure '{}' = {}\n", m.name.replace('\'', "''"), m.expression));
            if let Some(fmt) = &m.format {
                out.push_str(&format!("\tformatString: {fmt}\n"));
            }
            out.push('\n');
        }

        for (idx, rel) in self.relationships.iter().enumerate() {
            out.push_str(&format!(
                "relationship rel_{}\n\tfromColumn: {}.{}\n\ttoColumn: {}.{}\n\n",
                idx + 1,
                rel.from_table,
                rel.from_column,
                rel.to_table,
                rel.to_column
            ));
        }

        for role in &self.rls_roles {
            out.push_str(&format!("role '{}'\n", role.name.replace('\'', "''")));
            for pred in &role.filters {
                let vals = pred.allowed.join(", ");
                out.push_str(&format!("\ttablePermission {} = {}[{}] IN {{{}}}\n", pred.table, pred.table, pred.column, vals));
            }
            out.push('\n');
        }

        out
    }

    /// Parse a TMDL semantic model definition into a `ReportProject`.
    pub fn from_tmdl(default_title: impl Into<String>, tmdl_text: &str) -> Result<Self, ModelError> {
        let mut proj = ReportProject::new(default_title);
        let mut cur_table: Option<String> = None;
        let mut cur_measure: Option<String> = None;
        let mut cur_rel_from: Option<(String, String)> = None;
        let mut cur_rel_to: Option<(String, String)> = None;
        let mut cur_role: Option<RlsRole> = None;

        for raw_line in tmdl_text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("///") || line.starts_with("//") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("model ") {
                proj.title = rest.trim().trim_matches('\'').to_string();
                continue;
            }
            if let Some(rest) = line.strip_prefix("table ") {
                flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
                flush_role(&mut proj, &mut cur_role);
                let tname = rest.trim().trim_matches('\'').to_string();
                proj.tables.entry(tname.clone()).or_insert_with(|| Table { name: tname.clone(), columns: Vec::new() });
                cur_table = Some(tname);
                cur_measure = None;
                continue;
            }
            if let Some(rest) = line.strip_prefix("column ") {
                if let Some(tname) = &cur_table {
                    let cname = rest.trim().trim_matches('\'').to_string();
                    if let Some(t) = proj.tables.get_mut(tname) {
                        if t.column(&cname).is_none() {
                            t.columns.push(Column { name: cname, values: Vec::new() });
                        }
                    }
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("measure ") {
                flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
                flush_role(&mut proj, &mut cur_role);
                if let Some((lhs, rhs)) = rest.split_once('=') {
                    let name = lhs.trim().trim_matches('\'').to_string();
                    let expr = rhs.trim().to_string();
                    proj.add_measure(Measure { name: name.clone(), expression: expr, format: None })?;
                    cur_measure = Some(name);
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("formatString:") {
                if let Some(mname) = &cur_measure {
                    if let Some(m) = proj.measures.get_mut(mname) {
                        m.format = Some(rest.trim().to_string());
                    }
                }
                continue;
            }
            if line.starts_with("relationship ") {
                flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
                flush_role(&mut proj, &mut cur_role);
                cur_table = None;
                cur_measure = None;
                continue;
            }
            if let Some(rest) = line.strip_prefix("fromColumn:") {
                if let Some((t, c)) = rest.trim().split_once('.') {
                    cur_rel_from = Some((t.trim().to_string(), c.trim().to_string()));
                }
                continue;
            }
            if let Some(rest) = line.strip_prefix("toColumn:") {
                if let Some((t, c)) = rest.trim().split_once('.') {
                    cur_rel_to = Some((t.trim().to_string(), c.trim().to_string()));
                }
                flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
                continue;
            }
            if let Some(rest) = line.strip_prefix("role ") {
                flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
                flush_role(&mut proj, &mut cur_role);
                let rname = rest.trim().trim_matches('\'').to_string();
                cur_role = Some(RlsRole { name: rname, filters: Vec::new() });
                continue;
            }
            if let Some(rest) = line.strip_prefix("tablePermission ") {
                if let Some(role) = &mut cur_role {
                    if let Some((_tbl, expr)) = rest.split_once('=') {
                        if let Some((col_spec, in_part)) = expr.split_once("IN") {
                            if let Ok((table, column)) = crate::parse_table_col(col_spec.trim()) {
                                let inner = in_part.trim().trim_start_matches('{').trim_end_matches('}');
                                let allowed: Vec<String> = inner
                                    .split(',')
                                    .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                role.filters.push(FilterPredicate { table, column, allowed });
                            }
                        }
                    }
                }
            }
        }

        flush_rel(&mut proj, &mut cur_rel_from, &mut cur_rel_to);
        flush_role(&mut proj, &mut cur_role);
        Ok(proj)
    }
}

fn flush_rel(proj: &mut ReportProject, from: &mut Option<(String, String)>, to: &mut Option<(String, String)>) {
    if let (Some((ft, fc)), Some((tt, tc))) = (from.take(), to.take()) {
        let _ = proj.add_relationship(Relationship { from_table: ft, from_column: fc, to_table: tt, to_column: tc });
    }
}

fn flush_role(proj: &mut ReportProject, role: &mut Option<RlsRole>) {
    if let Some(r) = role.take() {
        proj.add_rls_role(r);
    }
}
