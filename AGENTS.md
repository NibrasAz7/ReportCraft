# ReportCraft — instructions for agents

ReportCraft is an independent, open-source, Rust-native business-intelligence
studio targeting Microsoft Power BI and SAP BusinessObjects workflows: data
connectors, a star-schema semantic model with a filter-context measure engine,
interactive dashboards, and paginated reports.

## Start every session here

1. Read [`ROADMAP.md`](ROADMAP.md) and [`vision.md`](vision.md).
2. Behaviour reference: public documentation and specifications only.

## Non-negotiables

- **Independent implementation:** never copy proprietary source code, binaries,
  connector libraries, or copyrighted assets from Microsoft Power BI, SAP
  BusinessObjects, or GPL/AGPL tools.
- **Never crash: no panics in non-test code.** Untrusted CSV/TSV files,
  `.reportcraft` projects, and measure expressions return `Result`; cap
  input-derived allocations and index with `.get()`.
- **Rust only.** No Electron or webview shells; keep core crates WASM-compatible.
- **Layering** (enforced by `cargo xtask layers`):
  `rc-model` (L0) → `rc-render` (L1) → `rc-engine` (L5) → `rc-mcp` (L6) →
  `apps/*` (L7). Nothing below L6 may depend on UI/windowing crates.
- **Everything is a command:** every action (`table.import_csv`,
  `model.relate`, `model.add_measure`, `model.query`, `visual.add`,
  `slicer.set`, `report.export_svg`, `report.export_html`) is registered in
  `rc-engine` and reachable identically from `reportcraft-cli` and `rc-mcp`.
- **Quality gates** before every commit: `cargo xtask ci` (fmt, clippy `-D
  warnings`, tests, layers).
