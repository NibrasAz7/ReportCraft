# ReportCraft — Vision

*(Working name — see "The name" below; final name to be confirmed.)*

A free, open-source business-intelligence studio — the clean-room,
Rust-native alternative to **Microsoft Power BI** and **SAP Business
Intelligence**: connect to your data, model it, and publish interactive
dashboards, charts, tables and reports.

> Built the **ArtCraft** way: the engineering conventions, layering and
> product standards of the [storytold](https://github.com/storytold)
> Craft app suite — an effort that follows that vision and contributes
> back to it.

## The problem

Business intelligence is where proprietary lock-in is deepest:

- **Power BI** is excellent — and it's a funnel: desktop is free, but
  sharing, refresh and collaboration all route through a per-seat
  Microsoft cloud subscription. Your semantic model lives in a closed
  `.pbix` container.
- **SAP BI / BusinessObjects** is enterprise-priced, enterprise-heavy,
  and effectively unreachable below large organizations.
- The credible open tools (Metabase, Apache Superset) are
  server-first — you run infrastructure before you draw a chart, and
  neither is a native desktop app with a real modelling surface.

Analysts deserve a tool that installs, opens, connects, and never asks
for a tenant.

## The product

**Connect → model → visualize → share.** ReportCraft is one native app
covering the whole BI pipeline:

| Stage | Capability | Replaces |
| --- | --- | --- |
| **Connect** | Wide connector library — SQL Server, PostgreSQL, MySQL, SQLite, Oracle, Excel/CSV/Parquet/JSON files, OData/REST — import *or* live query | Power BI "Get Data" |
| **Model** | Relationships between tables, calculated columns, a DAX-class measure language, columnar in-memory engine with query folding where the source supports it | Power BI semantic model |
| **Visualize** | Drag fields onto a report canvas: bar/line/scatter/pie, tables, matrices, cards, KPIs, maps; cross-filtering, drill-down, slicers | Power BI report view / SAP BO dashboards |
| **Share** | Pixel-accurate printed/exported reports (PDF), interactive HTML, scheduled refresh, publish to a self-hosted server | Power BI Service / SAP BI platform |

A dashboard tile, a table row and a printed report page are views of the
same semantic model — one document, no exports stitched between tools.

## The ArtCraft way

- **Rust only.** Native egui/eframe desktop app; the same Rust compiled
  to WebAssembly for the web. No Electron, no webview shell.
- **Clean-room.** Behaviour from public documentation and published
  specs — SQL dialects, the DAX language reference, file-format docs —
  never copied code, connectors or assets.
- **Never crash.** No panics, no `unsafe`; connector responses and
  malformed files are hostile input — every failure is a `Result` the
  analyst can act on.
- **Everything is a command.** Connect a source, define a measure, place
  a visual, refresh, export — all commands in the engine, reachable
  identically from the UI, the headless CLI, the control channel and
  MCP. Agents are first-class users: *"from `sales.xlsx`, model orders
  by region and product, build a dashboard, export PDF"* is one
  scriptable pipeline.
- **Local-first.** A project is a folder; the workbook and its model are
  yours. Offline by default — a tool that *reads databases* must never
  *be* a hostage to one vendor's cloud.
- **Open formats.** Native `.reportcraft` project plus CSV/JSON/Parquet
  dataset export and HTML/PDF report export. Leaving is free.

## What "complete" means

1. **Connect** — databases (SQL Server, Postgres, MySQL, SQLite,
   Oracle), files (Excel, CSV, TSV, Parquet, JSON), and web sources
   (OData, REST/JSON); credentials stay in the OS keychain.
2. **Model** — table relationships, calculated columns, measure
   language (aggregation, filter context, time intelligence), a
   columnar engine with SIMD-friendly scans and predicate pushdown.
3. **Visualize** — chart grammar covering bars, lines, areas, scatter,
   pies, treemaps, gauges, maps; tables and matrices with conditional
   formatting; slicers, drill-through, cross-filtering, tooltips.
4. **Report** — multi-page pixel-accurate reports with headers,
   parameters and pagination; PDF/PNG export.
5. **Refresh & share** — scheduled refresh, interactive standalone HTML
   export, a lightweight self-hostable server for shared dashboards.
6. **Govern** — row-level security, reusable datasets, certified/shared
   semantic models.

## Milestone sketch

- **M0 — Engine.** Columnar store, query execution, table/chart
  rendering, `.reportcraft` project file.
- **M1 — Connect.** CSV/TSV/Excel/SQLite first; SQL Server and Postgres
  live query next.
- **M2 — Canvas.** Report canvas, the core visual set, slicers and
  cross-filtering.
- **M3 — Model.** Relationships + the measure language (aggregation and
  filter context first; time intelligence after).
- **M4 — Reports & refresh.** Paginated reports, PDF export, scheduled
  refresh.
- **M5 — Share & web.** HTML publish, self-hosted server, WASM web app,
  packaging per the ArtCraft release conventions.

## The name — to be confirmed

Three candidates under consideration:

| Candidate | Pros | Cons |
| --- | --- | --- |
| **PowerCraft** | Instantly signals "Power BI alternative"; short, punchy | Generic — could be power-electronics or utility software; rides adjacent to Microsoft's "Power" family branding (Power BI/Apps/Platform), which risks confusion and trademark friction |
| **ReportCraft** | Fits the `{Function}Craft` convention cleanly; reports and dashboards are exactly what users open the tool for; short, unambiguous | Slightly undersells the product — the hard core is the semantic model and query engine, not the report canvas; "report" can sound like static documents rather than interactive BI |
| **IntelligenceCraft** | Literally "Business Intelligence" — the category is unmistakable; zero confusion about what it replaces | Long (17 chars), clumsy to say and type; "Intelligence" now carries AI connotations that invite a different misreading |

**Current leaning: ReportCraft** — it fits the family convention,
describes the artefact users actually produce, and avoids trademark
shadows. If the semantic-model scope feels too big for the name, the
product can still be positioned as "reports and dashboards" while the
engine does more underneath.

## The vision in one line

BI without the tenant, the seat count, or the gatekeeper: connect your
data, ask the question, publish the answer — on your own machine.
