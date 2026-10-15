//! `rc-ui-egui` (L6) — ReportCraft Desktop Studio UI.
//!
//! Three Power BI-class workspaces over one semantic model:
//! - **Report**: interactive slicer bar (`regions[region_name]`, `orders[quarter]`),
//!   DAX-class measure formula bar, KPI cards, and chart/table visual tiles.
//! - **Data**: tabular grid explorer over every imported table (`regions`,
//!   `products`, `orders`).
//! - **Model**: star-schema relationship diagram (`from_table[fk] -> to_table[pk]`)
//!   and measure dictionary.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

pub mod control;

use std::path::PathBuf;
use std::sync::mpsc::Receiver;

use base64::Engine as _;
use control::ControlRequest;
use rc_engine::Session;
use rc_model::VisualKind;
use serde_json::{json, Value as Json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StudioView {
    Report,
    Data,
    Model,
}

impl StudioView {
    pub fn label(self) -> &'static str {
        match self {
            StudioView::Report => "Report",
            StudioView::Data => "Data",
            StudioView::Model => "Model",
        }
    }
}

#[derive(Default)]
pub struct ReportAppOptions {
    pub control_rx: Option<Receiver<ControlRequest>>,
    pub startup_file: Option<PathBuf>,
}

pub struct ReportApp {
    pub session: Session,
    pub active_view: StudioView,
    pub selected_table: String,
    pub measure_name_input: String,
    pub measure_expr_input: String,
    pub active_region_slicer: String,
    pub status_line: String,
    control_rx: Option<Receiver<ControlRequest>>,
    pub(crate) pending_shots: Vec<(ControlRequest, Option<String>)>,
}

impl ReportApp {
    pub fn new(_cc: &eframe::CreationContext<'_>, opts: ReportAppOptions) -> Self {
        Self::with_options(opts)
    }

    pub fn with_options(opts: ReportAppOptions) -> Self {
        let mut session = Session::new();
        let mut status_line = String::from("Loaded Northwind Global Revenue & Margin star-schema model");
        if let Some(p) = &opts.startup_file {
            match session.execute("project.open", json!({"path": p.to_string_lossy()})) {
                Ok(_) => status_line = format!("Opened {}", p.display()),
                Err(e) => status_line = format!("Open error: {e}"),
            }
        }
        ReportApp {
            session,
            active_view: StudioView::Report,
            selected_table: "orders".to_string(),
            measure_name_input: "Avg Deal Size".to_string(),
            measure_expr_input: "AVERAGE(orders[revenue])".to_string(),
            active_region_slicer: "ALL".to_string(),
            status_line,
            control_rx: opts.control_rx,
            pending_shots: Vec::new(),
        }
    }

    pub fn exec_command(&mut self, cmd: &str, params: Json) -> Json {
        match self.session.execute(cmd, params) {
            Ok(v) => {
                self.status_line = format!("Executed {cmd}");
                json!({"ok": true, "result": v})
            }
            Err(e) => {
                self.status_line = format!("{cmd}: {e}");
                json!({"ok": false, "error": e.to_string()})
            }
        }
    }

    fn poll_control(&mut self, ctx: &egui::Context) {
        let mut batch = Vec::new();
        if let Some(rx) = &self.control_rx {
            while let Ok(req) = rx.try_recv() {
                batch.push(req);
            }
        }
        for req in batch {
            control::handle(self, ctx, req);
        }
    }

    fn poll_screenshots(&mut self, ctx: &egui::Context) {
        if self.pending_shots.is_empty() {
            return;
        }
        let shots: Vec<std::sync::Arc<egui::ColorImage>> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(std::sync::Arc::clone(image)),
                    _ => None,
                })
                .collect()
        });
        let Some(img) = shots.last() else { return };
        let mut png_bytes = Vec::new();
        let [w, h] = img.size;
        let encode_ok = {
            let mut enc = png::Encoder::new(&mut png_bytes, w as u32, h as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().and_then(|mut writer| writer.write_image_data(img.as_raw())).is_ok()
        };
        for (req, path) in std::mem::take(&mut self.pending_shots) {
            if !encode_ok {
                req.reply(json!({"ok": false, "error": "PNG encoding failed"}));
                continue;
            }
            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            let mut out = json!({"ok": true, "width": w, "height": h, "bytes": png_bytes.len(), "pngBase64": b64});
            if let Some(p) = path {
                match std::fs::write(&p, &png_bytes) {
                    Ok(()) => out["path"] = json!(p),
                    Err(e) => out = json!({"ok": false, "error": format!("write {p}: {e}")}),
                }
            }
            req.reply(out);
        }
    }

    fn draw_top_ribbon(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(&self.session.project().title);
            ui.separator();
            for v in [StudioView::Report, StudioView::Data, StudioView::Model] {
                if ui.selectable_label(self.active_view == v, v.label()).clicked() {
                    self.active_view = v;
                }
            }
            ui.separator();
            ui.label("Region Slicer:");
            for reg in ["ALL", "EMEA", "AMER", "APAC"] {
                if ui.selectable_label(self.active_region_slicer == reg, reg).clicked() {
                    self.active_region_slicer = reg.to_string();
                    let allowed: Vec<&str> = if reg == "ALL" { Vec::new() } else { vec![reg] };
                    let _ = self.exec_command("slicer.set", json!({"table": "regions", "column": "region_name", "allowed": allowed}));
                }
            }
            ui.separator();
            if ui.button("Reset Sample").clicked() {
                self.active_region_slicer = "ALL".to_string();
                let _ = self.exec_command("project.sample", json!({}));
            }
        });
    }

    fn draw_measure_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("fx Measure:");
            ui.add(egui::TextEdit::singleline(&mut self.measure_name_input).desired_width(140.0));
            ui.label("=");
            ui.add(egui::TextEdit::singleline(&mut self.measure_expr_input).desired_width(340.0));
            if ui.button("Define Measure").clicked() {
                let name = self.measure_name_input.trim().to_string();
                let expr = self.measure_expr_input.trim().to_string();
                let _ = self.exec_command("model.add_measure", json!({"name": name, "expression": expr}));
            }
        });
    }

    fn draw_report_view(&self, ui: &mut egui::Ui) {
        let proj = self.session.project();
        ui.horizontal_wrapped(|ui| {
            for (name, m) in &proj.measures {
                let val = proj.evaluate_measure(name, &[]).unwrap_or(0.0);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.set_min_width(165.0);
                    ui.label(egui::RichText::new(name).small().weak());
                    let formatted = if m.format.as_deref() == Some("percent") { format!("{:.1}%", val * 100.0) } else { format!("{:.2}", val) };
                    ui.heading(formatted);
                    ui.label(egui::RichText::new(&m.expression).monospace().small());
                });
            }
        });
        ui.add_space(10.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for v in &proj.visuals {
                if matches!(v.kind, VisualKind::KpiCard) {
                    continue;
                }
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.heading(&v.title);
                    if let Ok(q) = proj.query(v.group_by.as_deref(), &v.measures, &[]) {
                        egui::Grid::new(&v.id).striped(true).min_col_width(120.0).show(ui, |ui| {
                            ui.strong(q.group_column.as_deref().unwrap_or("Group"));
                            for m in &q.measure_names {
                                ui.strong(m);
                            }
                            ui.end_row();
                            for row in &q.rows {
                                ui.label(&row.group_key);
                                for val in &row.values {
                                    ui.monospace(format!("{val:.2}"));
                                }
                                ui.end_row();
                            }
                        });
                    }
                });
                ui.add_space(8.0);
            }
        });
    }

    fn draw_data_view(&mut self, ui: &mut egui::Ui) {
        let table_names: Vec<String> = self.session.project().tables.keys().cloned().collect();
        ui.horizontal(|ui| {
            ui.label("Tables:");
            for tname in &table_names {
                if ui.selectable_label(&self.selected_table == tname, tname).clicked() {
                    self.selected_table = tname.clone();
                }
            }
        });
        ui.separator();
        if let Some(table) = self.session.project().tables.get(&self.selected_table) {
            ui.label(format!("Table '{}' — {} rows × {} columns", table.name, table.row_count(), table.columns.len()));
            egui::ScrollArea::both().show(ui, |ui| {
                egui::Grid::new("rc_data_grid").striped(true).show(ui, |ui| {
                    for col in &table.columns {
                        ui.strong(&col.name);
                    }
                    ui.end_row();
                    for r in 0..table.row_count().min(200) {
                        for col in &table.columns {
                            let cell = col.values.get(r).map_or_else(String::new, |v| v.display_key());
                            ui.label(cell);
                        }
                        ui.end_row();
                    }
                });
            });
        }
    }

    fn draw_model_view(&self, ui: &mut egui::Ui) {
        let proj = self.session.project();
        ui.heading("Star-Schema Relationships");
        egui::Grid::new("rc_rels_grid").striped(true).show(ui, |ui| {
            ui.strong("Fact / Many Side");
            ui.strong("Direction");
            ui.strong("Dimension / One Side");
            ui.end_row();
            for rel in &proj.relationships {
                ui.monospace(format!("{}[{}]", rel.from_table, rel.from_column));
                ui.label("* ───> 1");
                ui.monospace(format!("{}[{}]", rel.to_table, rel.to_column));
                ui.end_row();
            }
        });
        ui.add_space(16.0);
        ui.heading("Semantic Measures");
        egui::Grid::new("rc_measures_grid").striped(true).show(ui, |ui| {
            ui.strong("Measure");
            ui.strong("DAX-Class Expression");
            ui.strong("Current Value");
            ui.end_row();
            for (name, m) in &proj.measures {
                ui.strong(name);
                ui.monospace(&m.expression);
                let val = proj.evaluate_measure(name, &[]).unwrap_or(0.0);
                ui.monospace(format!("{val:.2}"));
                ui.end_row();
            }
        });
    }
}

impl eframe::App for ReportApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_control(ctx);
        self.poll_screenshots(ctx);
        if self.control_rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("rc_top").show(ui, |ui| {
            self.draw_top_ribbon(ui);
            ui.separator();
            self.draw_measure_bar(ui);
        });
        egui::Panel::bottom("rc_status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.weak(&self.status_line);
            });
        });
        egui::CentralPanel::default().show(ui, |ui| match self.active_view {
            StudioView::Report => self.draw_report_view(ui),
            StudioView::Data => self.draw_data_view(ui),
            StudioView::Model => self.draw_model_view(ui),
        });
    }
}
