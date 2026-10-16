# ReportCraft

An independent, open-source, Rust-native business-intelligence studio
targeting **Microsoft Power BI** and **SAP BusinessObjects BI** workflows.

Connect to delimited and JSON files, clean and shape data through a
Power Query-style step pipeline with native SQL query folding, relate
tables in a star schema, write DAX-class filter-context and iterator
measures, enforce Row-Level Security (RLS), and publish interactive
dashboards, Power BI TMDL models, and multi-page paginated PDF reports —
locally, offline, and drivable via Studio UI, TCP control channel, CLI, or
MCP.

Built the [ArtCraft](https://github.com/storytold) way — the suite's
engineering conventions, not the suite's code.

## Status: Alpha (M0–M4 vertical slice)

**What works today:**

- **Columnar model, JSON/CSV I/O & Power Query pipeline (`rc-model`)**:
  typed columns (`Int`, `Float`, `Text`, `Bool`, `Null`), CSV/TSV and JSON
  record ingestion/export, step-based Power Query transformations
  (`FilterRows`, `SelectColumns`, `RenameColumn`, `AddCalculatedColumn`,
  `FillNull`), and **SQL query folding** (`fold_to_sql`) that compiles
  transformation pipelines into native `SELECT … FROM … WHERE …` queries.
- **Star-schema semantic model, DAX-class measures & RLS (`rc-model`)**:
  one-to-many star-schema relationships with cross-table slicer filter
  propagation; aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`,
  `DISTINCTCOUNT`, `DIVIDE`), row-context iterators (`SUMX`, `AVERAGEX`,
  `MINX`, `MAXX`, `COUNTX`), `CALCULATE(…, table[col] = val, ALL(…))`
  filter overrides, `TOTALYTD` time intelligence, and Row-Level Security
  (`RlsRole`) per viewer identity.
- **Power BI `.pbip` / TMDL interop (`rc-model::tmdl`)**: import and export
  semantic models (tables, columns, measures, relationships, and RLS roles)
  in Microsoft's text-based Tabular Model Definition Language (`.tmdl`).
- **SVG, standalone HTML, multi-page PDF 1.4 & bursting (`rc-render`)**:
  `Bar`, `Line`, `Area`, `Scatter`, `Pie`, `Treemap`, `Gauge`, `KpiCard`,
  `Table`, and `Matrix` visuals rendered to vector SVG, standalone HTML,
  multi-page paginated PDF 1.4 reports (`render_report_pdf`), and
  SAP BusinessObjects-style parameterized report bursting
  (`burst_report_html`).
- **Desktop Studio, CLI & MCP (`rc-ui-egui`, `rc-engine`, `rc-mcp`, `apps/*`)**:
  native `egui` Desktop Studio styled with `craft-ui` (`WidgetPalette`),
  persistent `config.toml` preferences via `craft-settings` (`ReportPrefs`),
  and crash-safe atomic file saves/exports via `craft-atomic` (`atomic_write`,
  from [`storytold/craft-libs`](https://github.com/storytold/craft-libs));
  **Report** (vector charts with click-to-cross-filter, KPI cards, `fx`
  measure bar), **Data** (table grid, calculated-column builder, live folded
  SQL preview), and **Model** (star-schema relationships, measures, TMDL
  preview) views; 23 engine commands reachable identically from Studio, TCP
  `--control PORT`, `reportcraft-cli`, and `reportcraft-cli mcp`.

**Known limitations (not yet implemented):**

- Compatibility is maintainer-reported and at an alpha stage.
- Live database drivers (PostgreSQL/SQL Server/MySQL/Oracle wire protocols;
  SQL query folding generates the native SQL today), Excel `.xlsx`/Parquet
  binary readers, and proprietary binary `.pbix` container extraction (use
  `.pbip`/`.tmdl` text format) are planned for M5 (see
  [`ROADMAP.md`](ROADMAP.md) and [`vision.md`](vision.md)).

## Quick start

```bash
cargo run -p reportcraft-cli -- demo
cargo run -p reportcraft-cli -- commands
cargo run -p reportcraft-cli -- export-pdf demo northwind.pdf
cargo run -p reportcraft-cli -- export-tmdl demo northwind.tmdl
cargo run -p reportcraft
cargo run -p xtask -- ci
```

## Layout

| Layer | Path | Role |
| --- | --- | --- |
| L0 | `crates/rc-model` | Columnar store, CSV/TSV/JSON I/O, Power Query transforms + SQL query folding, star-schema relationships, DAX-class measures (`SUMX`/`CALCULATE`/`TOTALYTD`), RLS roles, `.reportcraft` & Power BI `.tmdl` interop |
| L1 | `crates/rc-render` | Headless SVG, standalone HTML, multi-page paginated PDF 1.4 writer, and parameterized report bursting |
| L5 | `crates/rc-engine` | 23-command registry and guarded `Session` |
| L6 | `crates/rc-mcp` | JSON-RPC 2.0 MCP server over stdio |
| L6 | `crates/ui-egui` | Desktop Studio UI (Report, Data, Model views + click-to-cross-filter + TCP control dispatch) |
| L7 | `apps/reportcraft-cli` | Headless CLI (`demo`, `commands`, `--cmd`, `export-svg`, `export-html`, `export-pdf`, `export-tmdl`, `mcp`) |
| L7 | `apps/reportcraft` | Desktop Studio (`reportcraft [--control PORT] [file.reportcraft]`) |
| — | `xtask` | `cargo xtask ci` — `fmt`, `clippy -D warnings`, `test`, `layers` |

## Clean-room & License

MIT OR Apache-2.0 ([`LICENSE-MIT`](LICENSE-MIT),
[`LICENSE-APACHE`](LICENSE-APACHE)). Independently authored from public
documentation and specifications; no proprietary Microsoft or SAP source
code, binaries, or copyrighted assets are used.
