//! `pdf` — Pure-Rust multi-page PDF 1.4 vector report writer and
//! SAP BusinessObjects / SSRS-style parameterized report bursting.

use std::collections::BTreeMap;

use rc_model::{FilterPredicate, ModelError, ReportProject};

/// Render a multi-page paginated PDF 1.4 document for `project`.
/// Page 1 renders executive KPI summary tiles and a vector bar chart;
/// subsequent pages render paginated tabular visual breakdowns with
/// headers and `Page X of Y` footers.
pub fn render_report_pdf(project: &ReportProject) -> Result<Vec<u8>, ModelError> {
    let mut page_streams: Vec<String> = Vec::new();
    let total_pages = 1 + project.visuals.len().div_ceil(2).max(1);

    // Page 1: Executive header + KPI summary + first visual bar chart
    let mut p1 = String::new();
    draw_page_chrome(&mut p1, &project.title, 1, total_pages);

    let mut kx = 48.0;
    for (idx, (mname, m)) in project.measures.iter().take(4).enumerate() {
        let val = project.evaluate_measure(mname, &[]).unwrap_or(0.0);
        let disp = if m.format.as_deref() == Some("percent") { format!("{:.1}%", val * 100.0) } else { format!("{:.2}", val) };
        p1.push_str(&format!("0.95 0.97 1.00 rg {kx:.1} 650 120 56 re f\n"));
        p1.push_str(&format!("0.15 0.39 0.92 RG 1 w {kx:.1} 650 120 56 re S\n"));
        draw_text(&mut p1, kx + 8.0, 690.0, 9.0, mname);
        draw_text(&mut p1, kx + 8.0, 666.0, 14.0, &disp);
        kx += 132.0 + (idx as f64) * 0.0;
    }

    // Draw first grouped visual as a vector bar chart on Page 1
    if let Some(v) = project.visuals.iter().find(|v| v.group_by.is_some()) {
        let q = project.query(v.group_by.as_deref(), &v.measures, &[])?;
        draw_text(&mut p1, 48.0, 615.0, 12.0, &v.title);
        let max_v = q.rows.iter().filter_map(|r| r.values.first().copied()).fold(0.0_f64, f64::max).max(1.0);
        let chart_bottom = 410.0;
        let chart_h = 180.0;
        let slot_w = (480.0 / (q.rows.len().max(1) as f64)).min(110.0);
        let bar_w = (slot_w * 0.6).max(16.0);
        for (i, row) in q.rows.iter().enumerate() {
            let val = row.values.first().copied().unwrap_or(0.0).max(0.0);
            let bh = (val / max_v) * chart_h;
            let bx = 64.0 + (i as f64) * slot_w;
            p1.push_str(&format!("0.15 0.39 0.92 rg {bx:.1} {chart_bottom:.1} {bar_w:.1} {bh:.1} re f\n"));
            draw_text(&mut p1, bx, chart_bottom - 14.0, 9.0, &row.group_key);
            draw_text(&mut p1, bx, chart_bottom + bh + 6.0, 9.0, &format!("{val:.0}"));
        }
    }
    page_streams.push(p1);

    // Subsequent pages: 2 visuals per page rendered as paginated tables
    for (chunk_idx, chunk) in project.visuals.chunks(2).enumerate() {
        let page_no = chunk_idx + 2;
        let mut stream = String::new();
        draw_page_chrome(&mut stream, &project.title, page_no, total_pages);
        let mut cursor_y = 690.0;
        for v in chunk {
            let q = project.query(v.group_by.as_deref(), &v.measures, &[])?;
            draw_text(&mut stream, 48.0, cursor_y, 12.0, &v.title);
            cursor_y -= 20.0;
            let hdr_group = q.group_column.as_deref().unwrap_or("Group");
            draw_text(&mut stream, 56.0, cursor_y, 10.0, hdr_group);
            for (c, m) in q.measure_names.iter().enumerate() {
                draw_text(&mut stream, 200.0 + (c as f64) * 110.0, cursor_y, 10.0, m);
            }
            cursor_y -= 6.0;
            stream.push_str(&format!("0.80 0.84 0.90 RG 0.5 w 48 {cursor_y:.1} m 548 {cursor_y:.1} l S\n"));
            cursor_y -= 16.0;
            for row in q.rows.iter().take(12) {
                draw_text(&mut stream, 56.0, cursor_y, 10.0, &row.group_key);
                for (c, val) in row.values.iter().enumerate() {
                    draw_text(&mut stream, 200.0 + (c as f64) * 110.0, cursor_y, 10.0, &format!("{val:.2}"));
                }
                cursor_y -= 16.0;
            }
            cursor_y -= 24.0;
        }
        page_streams.push(stream);
    }

    Ok(assemble_pdf(&page_streams))
}

/// Parameterized report bursting: evaluates and renders a standalone HTML report
/// for each distinct value of `dimension_spec` (`table[column]`).
pub fn burst_report_html(project: &ReportProject, dimension_spec: &str) -> Result<BTreeMap<String, String>, ModelError> {
    let (table_name, col_name) = rc_model::parse_table_col(dimension_spec)?;
    let table = project.tables.get(&table_name).ok_or_else(|| ModelError::UnknownTable(table_name.clone()))?;
    let col = table.column(&col_name).ok_or_else(|| ModelError::UnknownColumn { table: table_name.clone(), column: col_name.clone() })?;

    let mut keys = std::collections::BTreeSet::new();
    for v in &col.values {
        keys.insert(v.display_key());
    }

    let mut out = BTreeMap::new();
    for key in keys {
        let mut sliced = project.clone();
        sliced.title = format!("{} — {} = {}", project.title, col_name, key);
        sliced.slicers.retain(|s| !(s.table == table_name && s.column.eq_ignore_ascii_case(&col_name)));
        sliced.slicers.push(FilterPredicate { table: table_name.clone(), column: col_name.clone(), allowed: vec![key.clone()] });
        let html = crate::render_report_html(&sliced)?;
        out.insert(key, html);
    }
    Ok(out)
}

fn draw_page_chrome(stream: &mut String, title: &str, page_no: usize, total_pages: usize) {
    stream.push_str("0.06 0.09 0.16 rg 0 742 612 50 re f\n");
    stream.push_str("1 1 1 rg\n");
    draw_text(stream, 48.0, 762.0, 14.0, title);
    stream.push_str("0.06 0.09 0.16 rg\n");
    draw_text(stream, 48.0, 32.0, 9.0, &format!("ReportCraft Paginated Report  |  Page {page_no} of {total_pages}"));
}

fn draw_text(stream: &mut String, x: f64, y: f64, size: f64, text: &str) {
    let escaped = text.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)");
    stream.push_str(&format!("BT /F1 {size:.1} Tf {x:.1} {y:.1} Td ({escaped}) Tj ET\n"));
}

fn assemble_pdf(page_streams: &[String]) -> Vec<u8> {
    // Object 1: Catalog
    // Object 2: Pages
    // Object 3: Font (/Helvetica)
    // For each page i (0..N):
    //   Object 4 + 2*i: Page dictionary
    //   Object 5 + 2*i: Content stream
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"%PDF-1.4\n");

    let mut offsets: Vec<usize> = Vec::new();
    let push_obj = |buf: &mut Vec<u8>, offs: &mut Vec<usize>, id: usize, body: &str| {
        offs.push(buf.len());
        let header = format!("{id} 0 obj\n{body}\nendobj\n");
        buf.extend_from_slice(header.as_bytes());
    };

    push_obj(&mut out, &mut offsets, 1, "<< /Type /Catalog /Pages 2 0 R >>");

    let kids: Vec<String> = (0..page_streams.len()).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    push_obj(&mut out, &mut offsets, 2, &format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), page_streams.len()));
    push_obj(&mut out, &mut offsets, 3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");

    for (i, content) in page_streams.iter().enumerate() {
        let page_id = 4 + 2 * i;
        let stream_id = 5 + 2 * i;
        let page_dict =
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {stream_id} 0 R >>");
        push_obj(&mut out, &mut offsets, page_id, &page_dict);
        let stream_obj = format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content);
        push_obj(&mut out, &mut offsets, stream_id, &stream_obj);
    }

    let xref_offset = out.len();
    let total_objs = offsets.len() + 1;
    out.extend_from_slice(format!("xref\n0 {total_objs}\n0000000000 65535 f \n").as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {total_objs} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes());
    out
}
