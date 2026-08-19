# frequent-contacts-ranker — competitor scan (2026-08-14)

Scan run BEFORE implementing, per `/create-next-tool` step 4. One WebSearch
("tool to find most frequent email contacts from mbox export rank by frequency and recency
top correspondents"), then the top real tools that actually do this job were skimmed.
Everything below is **paraphrased** — no competitor copy, branding, or trademarks are reused.

## Competitors reviewed

| # | Tool | Shape | What it actually does |
|---|------|-------|-----------------------|
| 1 | `mbox-sender-frequency` (open-source Python script, GitHub) | CLI script | Walks an mbox with Python's `mailbox` module and prints a count of messages grouped by **sender** address, sorted descending. Sold as a mailbox clean-up aid ("who is filling my quota"). No recency, no recipients, no export format. |
| 2 | `mailbox-report-generator` (PyPI) | CLI script | Analyses an mbox export (explicitly names Google Takeout as the source) and emits a report that includes a **most-active-addresses** section alongside volume-over-time stats. Report is a fixed text/HTML dump; no tuning knobs. |
| 3 | SysCurve MBOX Email Address Extractor | Paid Windows desktop app | Loads `.mbox` archives, harvests every address out of From/To/Cc/Bcc, de-duplicates, and exports to **CSV / JSON / XML / VCF / HTML / TXT**. Extraction + export breadth is the selling point; it does **not** rank by how often you talk to someone. |
| 4 | BitRecover MBOX Email Address Extractor | Paid Windows desktop app | Same class as #3: bulk address harvest from many mbox files at once, folder/batch mode, dedupe, CSV/TXT export, optional filter by domain. Ranking is absent; "filter by domain" and "remove duplicates" are the differentiators. |
| 5 | `comms-analyzer-toolbox` (open-source, Elasticsearch + Kibana) | Docker stack | Ingests mbox/CSV comms into Elasticsearch and gives Kibana dashboards for top senders/recipients over time, i.e. frequency **and** recency, but only via a self-hosted server stack. |
| — | Gmail-side "who do I email most" walkthroughs / analytics add-ons | SaaS add-on | Consumer answer to the same question: connect an account, get a top-contacts leaderboard with a time window. Requires an account + OAuth; out of model here. |

Two more were opened and discarded as not-competitors: general MBOX **viewer** apps with an
advanced-search screen (search, not ranking) and forensics blog posts that just describe running
the above scripts.

## Table stakes extracted (each one is either in the descriptor or listed out-of-model)

| Table stake | Seen in | Decision |
|---|---|---|
| Parse a real mbox (postmark-delimited, multi-message) and a lone `.eml` | 1,2,3,4,5 | **In** — `mail-parser`, shared split logic with `gmail-takeout-parser`/`mbox-splitter`. |
| Count by **sender** | 1,2,5 | **In** — `count = senders`. |
| Count by **recipient** (To/Cc/Bcc) — "people you email" | 3,4,5 | **In** — `count = recipients`, with `include_cc` to keep or drop Cc/Bcc. |
| Both directions in one ranking | 5 | **In** — `count = both` (the default), with per-direction `to`/`from` columns so the split stays visible. |
| Recency, not just raw volume | 5 (dashboards), consumer add-ons (time window) | **In** — exponential recency weighting via `half_life_days`; `0` disables it and gives pure frequency (what #1/#2 do). Reference "now" is the newest message in the archive, so results are deterministic and offline. |
| De-duplicate addresses | 3,4 | **In** — case-folded address key; one row per person. |
| Keep the display name | 3 (VCF export) | **In** — the most frequently used display name per address is kept, so `list` output is paste-ready `Name <addr>`. |
| Filter by / exclude a domain | 4 | **In** — `exclude` accepts addresses **and** `@domain` patterns (also how you drop your own address). |
| Drop automated/no-reply senders | (mailbox-cleanup framing of 1) | **In** — `skip_automated`, on by default. |
| Minimum-volume floor | 2 (report sections) | **In** — `min_messages`. |
| Top-N cut | 1,2,5 | **In** — `limit` (default 25, `0` = all). |
| Export CSV / JSON / plain address list | 3,4 | **In** — `format = report \| list \| csv \| json`. |
| Sort control | 1,2 | **In** — `sort = score \| messages \| recent \| name`. |
| Batch: many mbox files at once | 3,4 | **Out of model** — one text input per run; paste concatenated archives instead (they still split on postmarks). |
| VCF / XML / HTML export | 3,4 | **Out of model as such** — CSV+JSON cover the machine-readable need, and `blocks/csv-to-vcard` already turns the CSV into `.vcf`, so a fourth exporter would duplicate a shipped block. |
| Volume-over-time charts / dashboards | 2,5 | **Out of model** — needs a charting surface and a server stack; the page renders one text result. |
| Connect a live mailbox / OAuth | consumer add-ons | **Out of model** — gizza is browser-local, no accounts, no server. |

## UX decisions taken from the scan

- **Preset chips** (`[[example]]`) for the three jobs the competitors split across separate
  products: build an autocomplete list, find top senders to unsubscribe from, and export CSV.
- **Friendly `<select>` labels** (`[input.labels]`) on `count`/`sort`/`format` — the raw enum
  values read as jargon.
- **Slider** on `half_life_days` (0–730) because it is a bounded, exploratory dial; `0` at the
  left end is the "pure frequency" mode the script competitors ship.
- Aligned **rank table** as the default output (what #2's report gives) rather than raw counts,
  with the message total and date range in the header line so the ranking has context.
- Stated limits on the page: 5000-message cap, Bcc rarely present in exports, and the fact that
  the recency clock is the newest message in the paste.
