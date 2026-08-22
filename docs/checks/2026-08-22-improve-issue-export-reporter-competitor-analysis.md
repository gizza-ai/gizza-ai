# issue-export-reporter — competitor scan (2026-08-22)

Scan run BEFORE implementing. Everything below is **paraphrased** from public product
documentation and open-source READMEs — no competitor copy, branding, wording or
trademarks were reproduced, and none was used in the tool's page copy.

## Competitors reviewed

| # | Tool | What it is | Reached |
|---|------|-----------|---------|
| 1 | Atlassian Jira Cloud "cycle time report" (vendor documentation) | Built-in report on the hosted board | yes |
| 2 | `optilude/jira-cycle-extract` (open-source CLI) | Pulls issues from a Jira instance and emits flow-metric data files | yes |
| 3 | `marian-kamenistak/jira-lead-cycle-time-duration-extractor` (open-source CLI) | Exports per-status durations for downstream lead/cycle-time maths | yes |

A fourth candidate (a marketplace velocity-chart gadget vendor page) returned HTTP 403 to the
fetcher, so it was **replaced** rather than counted — the third profile above is a real,
reachable competitor in its place. A vendor blog comparing the marketplace "time in status" /
"time metrics" apps was read as secondary context for the app-family feature set.

## What they do (paraphrased)

**1. Vendor built-in report.** Measures the elapsed time of a work item over a defined
start→ship window; reports the **median**, not the mean; compares the selected week against a
12-week trailing baseline; shows a per-item scatter of the selected week annotated as
above/below the weekly median; offers a date-range selector and a sortable per-item table;
drops items whose elapsed time exceeds 12 months.

**2. `jira-cycle-extract`.** Configured with an ordered **workflow step list** (at least two
steps; several raw statuses may be collapsed onto one analytical state). Defines cycle time as
the days from entering the first *active* step (step 2 — step 1 is treated as an intake queue)
to entering the completed step. Filters by project, issue type and "valid resolutions".
Computes: cycle time per item, cumulative flow (daily count per state), a completion-date vs
cycle-time scatter, a histogram, **percentiles (defaults 30/50/70/85/95, overridable)**,
throughput over a rolling window (60 days by default, adjustable), burn-up, ageing WIP, and
weekly net flow (arrivals vs departures). Emits CSV (default), XLSX or JSON; PNG charts are an
optional extra requiring a plotting stack.

**3. `jira-lead-cycle-time-duration-extractor`.** YAML-configured field mapping (map arbitrary
export column names onto known fields, including custom fields), JQL filter, and an export
that automatically adds a per-status duration and per-status start timestamp so the user can
compute lead/cycle time downstream. Output as CSV or spreadsheet.

**Marketplace app family (secondary context).** Configurable status *groups*; per-assignee and
per-date breakdowns; start/stop/pause status selection; first-vs-last transition choice;
working-calendar support so non-working hours are excluded; spreadsheet export.

## Table stakes → in-model / out-of-model

| Table stake | Verdict | Where it landed |
|---|---|---|
| Status breakdown with counts and share of total | in-model | `report=status`, and the summary's status table (count, %, points) |
| Cycle time distribution, not just an average | in-model | min / percentiles / max / mean, all reported together |
| **Median-first** reporting (percentiles over means) | in-model | `percentiles` param, default `50,85,95`; mean is shown but secondary |
| Configurable percentiles | in-model | `percentiles` accepts any 1–99 list, e.g. `30,50,70,85,95` |
| Lead time vs cycle time distinguished | in-model | both computed: lead = created→resolved, cycle = started→resolved |
| Start of the cycle-time clock is configurable | in-model | auto-detected start column, overridable via `columns=started=<header>` |
| Column/field mapping for non-standard exports | in-model | one `columns` param (`status=State, resolved=Done At, points=Estimate`) |
| Which statuses count as finished is configurable | in-model | `done_statuses` (default list), matched case-insensitively |
| Cancelled/rejected work excluded from throughput | in-model | `cancelled_statuses`, excluded from completed/velocity/cycle time |
| Working-days (skip weekends) | in-model | `business_days` checkbox — Sat/Sun removed from every elapsed time |
| Group the breakdown by assignee / type / priority / sprint | in-model | `group_by` |
| Velocity per sprint (count **and** points) | in-model | `report=velocity`, `period=auto\|sprint\|week\|month\|day`, plus the average |
| Arrivals vs departures / burn-down over time | in-model | `report=burndown` — created, completed, net, open-at-end per period |
| Per-item table sorted by elapsed time | in-model | `report=items` (slowest first) |
| Spreadsheet-friendly export | in-model | `format=csv` (plus `json`, `markdown`, `text`) |
| Hours as well as days | in-model | `unit` |
| Handles both vendors' export dialects | in-model | Jira (`Issue key`, `Created`, `Resolved`, `Story Points`, repeated `Sprint` columns) and Linear (`ID`, `Completed`, `Cycle Name`, `Estimate`) auto-detected |
| Live connection to the tracker (URL + credentials/API token, JQL) | **out-of-model** | listed, not built — gizza blocks are browser-local, no accounts, no network |
| Rendered PNG/interactive charts (CFD, scatter, histogram) | **out-of-model** | listed, not built — this tool's surface is text/CSV/JSON/Markdown |
| Status-transition history (time *in* each status) | **out-of-model** | listed, not built — a plain CSV export has no transition log; only the timestamps present in the file can be used |
| Monte-Carlo delivery forecast | **out-of-model** | listed, not built — needs an RNG; block output must stay deterministic |
| Working-hour calendars / holiday sets / per-team calendars | **considered, rejected** | `business_days` covers the common weekend case; a full calendar model is a config surface out of proportion to a paste-a-CSV tool |
| Collapsing many raw statuses onto analytical states | **considered, rejected** | the `done_statuses`/`cancelled_statuses` lists cover the decision the metrics actually depend on; a general status→state mapping would double the parameter surface for a reporting (not modelling) tool |
| XLSX output | **considered, rejected** | `csv` opens in every spreadsheet; a second binary writer adds weight for no new information |

## UX control patterns adopted

- Percentiles-as-a-list text field rather than fixed checkboxes (competitor 2's `--quantiles`
  shape), so `30,50,70,85,95` is expressible.
- Enum `<select>`s with friendly labels for report/format/period/group-by/unit/delimiter —
  every fixed-choice parameter is `Param::enumv`, so the page renders a real dropdown.
- `[[example]]` preset chips for the three real starting points (a Jira export, a Linear
  export, and a velocity view), mirroring the "pick a report" affordance the hosted products
  give on load.
- Limits stated on the page (5 MB input, 50 000 rows) rather than discovered through an error.

## Decisions recorded

- **Median-first**: the summary leads with p50/p85/p95 because every competitor that reports a
  single number reports a median; the mean is printed alongside but not headlined.
- **Completed** = status in `done_statuses` **or** a non-empty resolved timestamp, minus
  anything in `cancelled_statuses`. This matches the "valid resolutions" idea without needing
  a resolution column, which Linear exports do not have.
- **Percentile method**: nearest-rank (the smallest observed value at or below which at least
  p% of the items fall). Stated on the page and in the parameter description so a number can
  be reproduced by hand.
