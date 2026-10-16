# ReportCraft Roadmap

## Current stage: Alpha (M0–M4 core vertical slice landed)

- [x] **M0 — Columnar Engine & Project Format (`rc-model`)**: typed columns (`Int`, `Float`, `Text`, `Bool`, `Null`), CSV/TSV/JSON ingestion and export, allocation bounds, deterministic `.reportcraft` JSON serialization, and built-in Northwind star-schema sample.
- [x] **M1 — Power Query Transform Pipeline & SQL Query Folding (`rc-model::transform`)**: step-based `FilterRows`, `SelectColumns`, `RenameColumn`, `AddCalculatedColumn`, and `FillNull` transformations, plus `fold_to_sql` compiling transform pipelines into native `SELECT … FROM … WHERE …` SQL.
- [x] **M2 — Studio Canvas & Cross-Filtering (`rc-render`, `rc-ui-egui`)**: `Bar`, `Line`, `Area`, `Scatter`, `Pie`, `Treemap`, `Gauge`, `KpiCard`, `Table`, and `Matrix` visuals; interactive `egui` Studio with vector chart painting, click-to-cross-filter on bars, region slicers, Data/Query-Folding view, Model/TMDL view, and TCP `--control PORT` server.
- [x] **M3 — Semantic Model, Iterators, Time Intelligence, RLS & TMDL (`rc-model`)**: one-to-many star-schema relationships, `SUM`/`AVERAGE`/`MIN`/`MAX`/`COUNT`/`DISTINCTCOUNT`/`DIVIDE`, row-context iterators (`SUMX`/`AVERAGEX`/`MINX`/`MAXX`/`COUNTX`), `CALCULATE(…, ALL(…))`, `TOTALYTD` time intelligence, Row-Level Security (`RlsRole`), and Power BI `.pbip`/`.tmdl` import/export.
- [x] **M4 — Paginated PDF Reports & Bursting (`rc-render::pdf`)**: pure-Rust multi-page PDF 1.4 vector report writer (`render_report_pdf`) with headers, footers, KPI cards, charts, and paginated tables, plus SAP BusinessObjects-style parameterized report bursting (`burst_report_html`).

## Next milestones

- [ ] **M5 — Live Database Drivers, Server & Web**: live wire-protocol connectors (PostgreSQL, SQL Server, SQLite), Excel `.xlsx`/Parquet binary readers, WASM browser build, and optional single-binary `reportcraft-server`.
