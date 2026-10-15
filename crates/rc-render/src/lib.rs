//! `rc-render` — Headless SVG and standalone interactive HTML report renderer
//! for ReportCraft visuals (`Bar`, `Line`, `Pie`, `KpiCard`, `Table`).
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]

use rc_model::{ModelError, QueryResult, ReportProject, VisualKind, VisualSpec};

const PALETTE: &[&str] = &["#2563eb", "#0d9488", "#d97706", "#7c3aed", "#dc2626", "#059669"];

pub fn render_visual_svg(spec: &VisualSpec, result: &QueryResult, width: u32, height: u32) -> String {
    let w = width.max(240) as f64;
    let h = height.max(160) as f64;
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="Inter, system-ui, sans-serif">"##
    );
    svg.push_str(&format!(r##"<rect x="0" y="0" width="{w}" height="{h}" rx="6" fill="#ffffff" stroke="#e2e8f0" stroke-width="1"/>"##));
    svg.push_str(&format!(r##"<text x="16" y="28" font-size="14" font-weight="600" fill="#0f172a">{}</text>"##, escape_xml(&spec.title)));

    match spec.kind {
        VisualKind::KpiCard => render_kpi_body(&mut svg, result, w, h),
        VisualKind::Bar => render_bar_body(&mut svg, result, w, h),
        VisualKind::Line => render_line_body(&mut svg, result, w, h),
        VisualKind::Pie => render_pie_body(&mut svg, result, w, h),
        VisualKind::Table => render_table_body(&mut svg, result, w, h),
    }

    svg.push_str("</svg>");
    svg
}

pub fn render_report_svg(project: &ReportProject) -> Result<String, ModelError> {
    let cols = 2usize;
    let tile_w = 460u32;
    let tile_h = 280u32;
    let gap = 20u32;
    let n = project.visuals.len().max(1);
    let rows = n.div_ceil(cols);
    let total_w = (cols as u32) * tile_w + ((cols + 1) as u32) * gap;
    let total_h = 64 + (rows as u32) * tile_h + ((rows + 1) as u32) * gap;

    let mut out = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w} {total_h}" width="{total_w}" height="{total_h}" font-family="Inter, system-ui, sans-serif">"##
    );
    out.push_str(&format!(r##"<rect x="0" y="0" width="{total_w}" height="{total_h}" fill="#f8fafc"/>"##));
    out.push_str(&format!(r##"<text x="{gap}" y="40" font-size="20" font-weight="700" fill="#0f172a">{}</text>"##, escape_xml(&project.title)));

    for (idx, v) in project.visuals.iter().enumerate() {
        let q = project.query(v.group_by.as_deref(), &v.measures, &[])?;
        let col = (idx % cols) as u32;
        let row = (idx / cols) as u32;
        let x = gap + col * (tile_w + gap);
        let y = 64 + gap + row * (tile_h + gap);
        let tile = render_visual_svg(v, &q, tile_w, tile_h);
        out.push_str(&format!(r##"<g transform="translate({x},{y})">{tile}</g>"##));
    }

    out.push_str("</svg>");
    Ok(out)
}

pub fn render_report_html(project: &ReportProject) -> Result<String, ModelError> {
    let mut cards_html = String::new();
    for v in &project.visuals {
        let q = project.query(v.group_by.as_deref(), &v.measures, &[])?;
        let svg = render_visual_svg(v, &q, 480, 300);
        cards_html.push_str(&format!(r##"<div class="rc-tile" data-visual-id="{}">{svg}</div>"##, escape_xml(&v.id)));
    }
    let slicer_chips: Vec<String> = project
        .slicers
        .iter()
        .map(|s| {
            format!(r##"<span class="rc-chip">{}.{} = {}</span>"##, escape_xml(&s.table), escape_xml(&s.column), escape_xml(&s.allowed.join(", ")))
        })
        .collect();
    let slicers_bar =
        if slicer_chips.is_empty() { String::from(r##"<span class="rc-chip">All rows (no active slicers)</span>"##) } else { slicer_chips.join(" ") };

    Ok(format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — ReportCraft</title>
<style>
  body {{ margin: 0; padding: 24px; background: #f8fafc; color: #0f172a; font-family: Inter, system-ui, sans-serif; }}
  header {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }}
  h1 {{ margin: 0; font-size: 22px; }}
  .rc-slicers {{ margin-bottom: 20px; display: flex; gap: 8px; flex-wrap: wrap; }}
  .rc-chip {{ background: #e2e8f0; color: #1e293b; border-radius: 4px; padding: 4px 10px; font-size: 12px; font-family: monospace; }}
  .rc-grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(460px, 1fr)); gap: 20px; }}
  .rc-tile svg {{ width: 100%; height: auto; display: block; }}
</style>
</head>
<body>
<header>
  <h1>{title}</h1>
  <small>ReportCraft standalone interactive export</small>
</header>
<div class="rc-slicers">{slicers_bar}</div>
<main class="rc-grid">
{cards_html}
</main>
</body>
</html>"##,
        title = escape_xml(&project.title)
    ))
}

fn render_kpi_body(svg: &mut String, result: &QueryResult, w: f64, h: f64) {
    let val = result.rows.first().and_then(|r| r.values.first()).copied().unwrap_or(0.0);
    let label = result.measure_names.first().map_or("", String::as_str);
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" text-anchor="middle" font-size="36" font-weight="700" fill="#2563eb">{}</text>"##,
        w * 0.5,
        h * 0.56,
        format_num(val)
    ));
    svg.push_str(&format!(
        r##"<text x="{}" y="{}" text-anchor="middle" font-size="12" fill="#64748b">{}</text>"##,
        w * 0.5,
        h * 0.74,
        escape_xml(label)
    ));
}

fn render_bar_body(svg: &mut String, result: &QueryResult, w: f64, h: f64) {
    let pad_l = 48.0;
    let pad_r = 20.0;
    let pad_t = 46.0;
    let pad_b = 36.0;
    let plot_w = (w - pad_l - pad_r).max(40.0);
    let plot_h = (h - pad_t - pad_b).max(40.0);
    let max_val = result.rows.iter().filter_map(|r| r.values.first().copied()).fold(0.0_f64, f64::max).max(1.0);
    let n = result.rows.len().max(1);
    let slot_w = plot_w / (n as f64);
    let bar_w = (slot_w * 0.65).max(8.0);

    for (i, row) in result.rows.iter().enumerate() {
        let v = row.values.first().copied().unwrap_or(0.0).max(0.0);
        let bh = (v / max_val) * plot_h;
        let x = pad_l + (i as f64) * slot_w + (slot_w - bar_w) * 0.5;
        let y = pad_t + plot_h - bh;
        let color = PALETTE.get(i % PALETTE.len()).copied().unwrap_or("#2563eb");
        svg.push_str(&format!(r##"<rect x="{x:.1}" y="{y:.1}" width="{bar_w:.1}" height="{bh:.1}" rx="3" fill="{color}"/>"##));
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="11" fill="#334155">{}</text>"##,
            x + bar_w * 0.5,
            pad_t + plot_h + 18.0,
            escape_xml(&row.group_key)
        ));
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" font-size="10" fill="#475569">{}</text>"##,
            x + bar_w * 0.5,
            (y - 4.0).max(pad_t + 10.0),
            format_num(v)
        ));
    }
}

fn render_line_body(svg: &mut String, result: &QueryResult, w: f64, h: f64) {
    let pad_l = 48.0;
    let pad_r = 20.0;
    let pad_t = 46.0;
    let pad_b = 36.0;
    let plot_w = (w - pad_l - pad_r).max(40.0);
    let plot_h = (h - pad_t - pad_b).max(40.0);
    let max_val = result.rows.iter().filter_map(|r| r.values.first().copied()).fold(0.0_f64, f64::max).max(1.0);
    let n = result.rows.len();
    if n == 0 {
        return;
    }
    let mut pts = Vec::with_capacity(n);
    for (i, row) in result.rows.iter().enumerate() {
        let v = row.values.first().copied().unwrap_or(0.0);
        let x = if n == 1 { pad_l + plot_w * 0.5 } else { pad_l + (i as f64 / (n - 1) as f64) * plot_w };
        let y = pad_t + plot_h - (v / max_val) * plot_h;
        pts.push((x, y, &row.group_key));
    }
    let poly: Vec<String> = pts.iter().map(|(x, y, _)| format!("{x:.1},{y:.1}")).collect();
    svg.push_str(&format!(r##"<polyline fill="none" stroke="#2563eb" stroke-width="2.5" points="{}"/>"##, poly.join(" ")));
    for (x, y, label) in pts {
        svg.push_str(&format!(r##"<circle cx="{x:.1}" cy="{y:.1}" r="4" fill="#2563eb"/>"##));
        svg.push_str(&format!(
            r##"<text x="{x:.1}" y="{:.1}" text-anchor="middle" font-size="11" fill="#334155">{}</text>"##,
            pad_t + plot_h + 18.0,
            escape_xml(label)
        ));
    }
}

fn render_pie_body(svg: &mut String, result: &QueryResult, w: f64, h: f64) {
    let cx = w * 0.36;
    let cy = h * 0.56;
    let r = (w.min(h) * 0.30).max(24.0);
    for (i, row) in result.rows.iter().enumerate() {
        let v = row.values.first().copied().unwrap_or(0.0).max(0.0);
        let color = PALETTE.get(i % PALETTE.len()).copied().unwrap_or("#2563eb");
        let ly = 56.0 + (i as f64) * 22.0;
        svg.push_str(&format!(r##"<rect x="{:.1}" y="{:.1}" width="12" height="12" rx="2" fill="{color}"/>"##, w * 0.64, ly - 10.0));
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="{ly:.1}" font-size="12" fill="#334155">{}: {}</text>"##,
            w * 0.64 + 18.0,
            escape_xml(&row.group_key),
            format_num(v)
        ));
    }
    svg.push_str(&format!(r##"<circle cx="{cx:.1}" cy="{cy:.1}" r="{r:.1}" fill="none" stroke="#2563eb" stroke-width="18"/>"##));
}

fn render_table_body(svg: &mut String, result: &QueryResult, w: f64, _h: f64) {
    let start_y = 54.0;
    let row_h = 22.0;
    let cols = 1 + result.measure_names.len();
    let col_w = ((w - 32.0) / (cols as f64)).max(60.0);
    let first_hdr = result.group_column.as_deref().unwrap_or("Group");
    svg.push_str(&format!(r##"<text x="16" y="{start_y}" font-size="11" font-weight="700" fill="#475569">{}</text>"##, escape_xml(first_hdr)));
    for (c, m) in result.measure_names.iter().enumerate() {
        let x = 16.0 + ((c + 1) as f64) * col_w;
        svg.push_str(&format!(r##"<text x="{x:.1}" y="{start_y}" font-size="11" font-weight="700" fill="#475569">{}</text>"##, escape_xml(m)));
    }
    for (r_idx, row) in result.rows.iter().take(8).enumerate() {
        let y = start_y + ((r_idx + 1) as f64) * row_h;
        svg.push_str(&format!(r##"<text x="16" y="{y:.1}" font-size="11" fill="#0f172a">{}</text>"##, escape_xml(&row.group_key)));
        for (c_idx, v) in row.values.iter().enumerate() {
            let x = 16.0 + ((c_idx + 1) as f64) * col_w;
            svg.push_str(&format!(r##"<text x="{x:.1}" y="{y:.1}" font-size="11" fill="#0f172a">{}</text>"##, format_num(*v)));
        }
    }
}

fn format_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{:.0}", v)
    } else {
        format!("{:.2}", v)
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_model::{Measure, Table};

    #[test]
    fn renders_report_svg_and_html() {
        let mut p = ReportProject::new("Q4 Sales Report");
        let t = Table::from_delimited("sales", "region,revenue\nEMEA,1200\nAMER,2000\n", ',').unwrap();
        p.add_table(t);
        p.add_measure(Measure { name: "Revenue".into(), expression: "SUM(sales[revenue])".into(), format: None }).unwrap();
        p.visuals.push(VisualSpec {
            id: "v1".into(),
            title: "Revenue by Region".into(),
            kind: VisualKind::Bar,
            group_by: Some("sales[region]".into()),
            measures: vec!["Revenue".into()],
        });

        let svg = render_report_svg(&p).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Revenue by Region"));
        assert!(svg.contains("EMEA"));

        let html = render_report_html(&p).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Q4 Sales Report"));
    }
}
