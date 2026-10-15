# ReportCraft Roadmap

## Current stage: M0 — Columnar Engine, Filter-Context Measures & Headless Reports

- [x] **M0.1 — Columnar store (`rc-model`)**: typed columns (`Int`, `Float`, `Text`, `Bool`, `Null`), CSV/TSV ingestion with type inference and allocation caps, deterministic `.reportcraft` JSON serialization.
- [x] **M0.2 — Star-schema relationships & filter context (`rc-model`)**: one-to-many table relationships, slicer context propagation across related dimension/fact tables.
- [x] **M0.3 — Measure engine (`rc-model`)**: `SUM`, `AVERAGE`, `MIN`, `MAX`, `COUNT`, `DISTINCTCOUNT`, `DIVIDE`, and `CALCULATE(expr, table[col] = literal, ...)` context overrides; grouped visual queries.
- [x] **M0.4 — Headless SVG & standalone HTML renderer (`rc-render`)**: bar, line, pie, KPI card, and table visuals rendered to vector SVG and self-contained interactive HTML with slicer controls.
- [x] **M0.5 — Command engine, CLI & MCP (`rc-engine`, `rc-mcp`, `reportcraft-cli`)**: unified command registry, headless CLI (`query`, `export-svg`, `export-html`, `--cmd`, `commands`, `mcp`), and JSON-RPC 2.0 MCP server over stdio.

## Next milestones

- [ ] **M1 — Connectors**: Excel (`.xlsx`), Parquet, SQLite, PostgreSQL, and SQL Server import/live query.
- [ ] **M2 — Studio Canvas (`ui-egui`)**: interactive report canvas, field well drag-and-drop, cross-filtering, and TCP control channel.
- [ ] **M3 — Expanded DAX-class semantics**: row-context iterators (`SUMX`, `FILTER`), time intelligence (`YTD`, `SAMEPERIODLASTYEAR`), `.pbip`/TMDL text model import.
- [ ] **M4 — Paginated reports & PDF**: multi-page headers/footers, parameterized bursting, native PDF writer.
- [ ] **M5 — Sharing & Web**: WASM browser build, optional single-binary server with row-level security.
