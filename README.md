# ReportCraft

An independent, open-source, Rust-native business-intelligence studio
targeting **Microsoft Power BI** and **SAP BusinessObjects BI** workflows.

Connect to delimited files and databases, relate tables in a star schema,
define filter-context measures, and render interactive dashboards, charts,
KPI cards, and tables — locally, offline, and drivable via Studio UI, TCP
control channel, CLI, or MCP.

Built the [ArtCraft](https://github.com/storytold) way — the suite's
engineering conventions, not the suite's code.

## Status: Alpha (M0–M2 core vertical slice)

**What works today:**

- **Columnar model & star-schema relationships (`rc-model`)**: typed columns
  (`Int`, `Float`, `Text`, `Bool`, `Null`), CSV/TSV ingestion with automatic
  type inference and allocation bounds, one-to-many star-schema relationships,
  slicer filter propagation across dimension and fact tables, a built-in
  Northwind star-schema sample model, and text-native `.reportcraft` JSON
  serialization.
- **Filter-context measure engine (`rc-model`)**: `SUM`, `AVERAGE`, `MIN`,
  `MAX`, `COUNT`, `DISTINCTCOUNT`, `DIVIDE`, arithmetic combinations, and
  `CALCULATE(expr, table[col] = value, ALL(table[col]))` context overrides;
  grouped queries over related tables.
- **Headless SVG & standalone HTML renderer (`rc-render`)**: `Bar`, `Line`,
  `Pie`, `KpiCard`, and `Table` visuals rendered to vector SVG or a
  self-contained interactive HTML report.
- **Desktop Studio (`rc-ui-egui`, `apps/reportcraft`)**: native `egui`
  Studio with **Report**, **Data**, and **Model** views, interactive region
  slicers, a `fx` DAX-class measure bar, and a TCP JSON-lines control
  channel (`reportcraft --control PORT`) supporting `ping`, `ui.state`,
  `ui.render`, `ui.screenshot`, `engine.execute`, and `engine.list`.
- **Command engine, CLI & MCP (`rc-engine`, `rc-mcp`, `reportcraft-cli`)**:
  15 engine commands (`project.sample`, `table.import_csv`, `model.relate`,
  `model.add_measure`, `model.query`, `visual.add`, `slicer.set`,
  `report.export_svg`, `report.export_html`, `project.save`, `project.open`,
  etc.) reachable identically from the Studio UI, `reportcraft-cli`, and the
  stdio JSON-RPC 2.0 MCP server.

**Known limitations (not yet implemented):**

- Compatibility is maintainer-reported and at an alpha stage.
- Live SQL/warehouse connectors (PostgreSQL, SQL Server, MySQL, Oracle,
  OData), Excel `.xlsx`/Parquet readers, and query folding are not yet
  implemented.
- Row-context iterators (`SUMX`, `FILTER`), time-intelligence functions,
  `.pbix`/`.pbip`/TMDL import, and paginated PDF reports are planned for
  M3–M5 (see [`ROADMAP.md`](ROADMAP.md) and [`vision.md`](vision.md)).

## Quick start

```bash
cargo run -p reportcraft-cli -- demo
cargo run -p reportcraft-cli -- commands
cargo run -p reportcraft
cargo run -p xtask -- ci
```

## Layout

| Layer | Path | Role |
| --- | --- | --- |
| L0 | `crates/rc-model` | Columnar table store, CSV/TSV I/O, star-schema relationships, filter-context measure engine, `.reportcraft` format |
| L1 | `crates/rc-render` | Headless SVG & standalone HTML visual/report renderer |
| L5 | `crates/rc-engine` | 15-command registry and guarded `Session` |
| L6 | `crates/rc-mcp` | JSON-RPC 2.0 MCP server over stdio |
| L6 | `crates/ui-egui` | Desktop Studio UI (Report, Data, Model views + TCP control dispatch) |
| L7 | `apps/reportcraft-cli` | Headless CLI (`demo`, `commands`, `--cmd`, `export-svg`, `export-html`, `mcp`) |
| L7 | `apps/reportcraft` | Desktop Studio (`reportcraft [--control PORT] [file.reportcraft]`) |
| — | `xtask` | `cargo xtask ci` — `fmt`, `clippy -D warnings`, `test`, `layers` |

## Clean-room & License

MIT OR Apache-2.0 ([`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE)). Independently authored from public
documentation and specifications; no proprietary Microsoft or SAP source
code, binaries, or copyrighted assets are used.
