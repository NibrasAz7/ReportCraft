# ReportCraft — Vision

Working name — see "The name" below; final name to be confirmed.

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

## Research: feature landscape

What Power BI and SAP BI actually ship, what open source covers, and
where the gap is.

- **Power BI** — 200+ connectors (files, every major RDBMS/lakehouse,
  SaaS, OData/REST, plus a custom-connector SDK); Power Query/M with
  **query folding** (push transforms into native SQL); the tabular
  **semantic model + DAX** (~250 functions built on row vs. filter
  context, `CALCULATE` context transition, time intelligence);
  **VertiPaq** in-memory columnar engine; ~35 core visuals + an
  AppSource custom-visual marketplace; paginated reports (SSRS
  lineage); the Service: workspaces, apps, scheduled/incremental
  refresh, RLS, endorsement, audit.
- **SAP BI / BusinessObjects** — universes (the original governed
  semantic layer), Web Intelligence ad-hoc, Crystal Reports
  pixel-perfect operational reports, and the platform's real value:
  scheduling, publication/**bursting**, folder security, audit.
- **Open source today** — Metabase (best business-user UX, thin
  semantic layer, governance paywalled, AGPL), Apache Superset
  (feature-rich but needs a deployed stack and SQL-fluent users),
  Lightdash (dbt-native, git-friendly, no non-dbt path), Evidence
  (BI-as-code, static narrative), Grafana (time-series only).

**The gap nobody fills:** everything open is a web app over a
warehouse — there is no native desktop authoring app, no DAX-class
filter-context engine, no VertiPaq-class local columnar store, no real
paginated reporting, and no `.pbix` interop.

### Hardest problems, honestly ranked

1. **A DAX-class measure engine** — filter context, context transition,
   time intelligence. Research-grade; the moat.
2. **Query folding** across dialects — translate visual steps into
   native `SELECT`s; required for live-query mode.
3. **Columnar performance** — dictionary/RLE-compressed in-memory scans
   (DuckDB/Polars/Arrow as the base).
4. **`.pbix` interop** — proprietary binary; the pragmatic path is
   `.pbip`/TMDL text formats and community extractors.
5. Gateway/refresh infrastructure and NLQ quality.

## Research: user journeys

Six personas, and what each demands.

- **Data analyst / BI developer.** Spreadsheet-to-dashboard: connect
  Excel/CSV + a SQL source → clean in a step-based query editor →
  relate tables → write measures → compose the dashboard → publish.
  Also: live-connect to a shared certified model instead of remodelling,
  and iterate under version control — *Power BI added `.pbip`/TMDL
  precisely because `.pbix` binaries can't be diffed; a text-based
  project format leapfrogs that from day one.*
- **Business user / consumer.** Open a shared dashboard → filter,
  cross-highlight, drill-through → export a view; subscribe to
  scheduled email/Slack snapshots or threshold alerts; self-serve a
  visual query builder or NL question. *Consumers outnumber authors
  10–100× — per-seat pricing on viewers is the dominant cost complaint
  and this product's wedge.*
- **Data engineer / modeler.** Build the governed semantic layer (star
  schema, hierarchies, measures) → publish a model-only artifact that
  analysts live-connect to → configure scheduled/incremental refresh
  and storage mode (import vs live query) per source.
- **BI admin.** Certify a model (promoted → certified, ranked higher in
  discovery); define RLS roles with filter expressions and "view as
  role" testing; audit views/edits/refreshes/shares. *All unpaid — the
  competitors gate exactly this.*
- **Executive viewer.** A scheduled KPI snapshot (email/PDF) or a
  single-page mobile-friendly dashboard; one level of drill; zero
  install.
- **Paginated-report consumer.** Parameterized operational documents —
  invoice runs, statements — rendered pixel-perfect to PDF, including
  scheduled bursting to many recipients. *This is the Crystal/SSRS
  niche nobody open-source serves well.*

## Research: collaboration model

BI collaboration is *sharing and governance*, not multiplayer editing —
different problem, different design.

**Recommendation: text-native project format + zero-infra HTML export +
optional single-binary server.**

- **Solo mode:** a project is a folder — `.reportcraft` manifest +
  `model/*.toml` + `reports/*.toml` + a data cache — with deterministic
  serialization so **git diffs and merges actually work** (the thing
  `.pbix` can't do). Credentials in the OS keychain/env, never in
  files. Fully offline.
- **Sharing without infrastructure:** **interactive HTML export** —
  the WASM-compiled engine plus an embedded data extract produces a
  self-contained `.html` with live filters and cross-highlighting.
  Power BI can't do this at any price. Static PNG/PDF for documents.
- **Team mode:** optional `reportcraft-server` — one Rust binary
  (axum + the same engine). `rc publish` pushes a versioned,
  content-addressed snapshot; workspaces-lite roles
  (Viewer/Curator/Admin) on collections; scheduled refresh with
  server-held service-account credentials; **RLS rules defined in the
  model files**, enforced by the engine per identity; revocable guest
  links; audit log. Comments and subscriptions live server-side.
- **Minimal governance:** Promoted/Certified flags on shared models,
  model-version pinning per report, `rc model diff` for reviewers.
  Defer SAP-style ACL sprawl.

**Known risks:** credentials are the hardest local-first problem
(per-user keychain in solo, service accounts in team mode); someone
must own refresh — honest docs plus HTML export (frozen data) as the
escape hatch; guest links and HTML export bypass RLS by construction —
block or warn loudly, following Power BI's lead; keep the server on
the same project format and engine so solo and team never diverge.

## The vision in one line

BI without the tenant, the seat count, or the gatekeeper: connect your
data, ask the question, publish the answer — on your own machine.
