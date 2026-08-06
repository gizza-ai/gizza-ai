# job-posting-parser — competitor / reference scan (2026-08-06)

Scan run **before** implementing, per `/create-next-tool` step 3. Everything below is
**paraphrased**; no competitor copy, branding, or trademarked wording was reused. Sources are
listed for provenance only.

## Sources skimmed

| # | Reference | What it is | URL |
|---|-----------|-----------|-----|
| 1 | Affinda — AI Job Description Parser | Commercial account/API JD parser (typed JSON out) | https://www.affinda.com/job-description-parser/ |
| 2 | Propellum — "Job Description Parsing Explained" | Vendor reference article defining the standard JD-parsing field set + normalization stages | https://www.propellum.com/blog/job-description-parsing-explained/ |
| 3 | schema.org `JobPosting` (the Google-for-Jobs structured-data contract) | The de-facto output schema every JD parser targets | https://schema.org/JobPosting |
| — | Apify / Browse AI job-listing scrapers (LinkedIn, CareerBuilder, Monster) | Account-based scrapers; skimmed only for their advertised per-listing field lists | search results |

## Table-stakes capabilities observed

| # | Capability (paraphrased) | Seen in | Fit | Where it lands here |
|---|--------------------------|---------|-----|---------------------|
| 1 | Job **title** extraction | 1,2,3, scrapers | **in-model** | `title` field |
| 2 | **Company / hiring organization** | 2,3, scrapers | **in-model** | `company` field |
| 3 | **Location** (city/region/country as written) | 2,3, scrapers | **in-model** | `location` field |
| 4 | **Salary / pay range** with currency, min–max, and period | 1,2,3, scrapers | **in-model** | `salary` object: `raw`, `currency`, `min`, `max`, `period` |
| 5 | **Salary normalization** to a comparable basis (e.g. `£60k–£75k DOE` → structured) | 2 | **in-model** | `annualize` param → `annual_min`/`annual_max` (40 h/wk × 52 wk assumption, stated on the page) |
| 6 | **Skills list** | 1,2,3, scrapers | **in-model** | `skills`, matched against a built-in ~300-entry vocabulary with aliases |
| 7 | **Required vs preferred** skill split | 2 | **in-model** | section-aware bucketing → `skills_required` / `skills_preferred` (`split_skills` param) |
| 8 | **Employment type** (full-time / part-time / contract / temporary / internship / seasonal / volunteer) | 2,3 | **in-model** | `employment_type`, normalized to the schema.org value set |
| 9 | **Seniority / experience level** | 1,2 | **in-model** | `seniority` (intern → C-level) |
| 10 | **Years of experience required** | 2 | **in-model** | `experience_years` (`5+ years` → 5) |
| 11 | **Education requirement** | 2 (qualifications) | **in-model** | `education` (high-school → PhD) |
| 12 | **Remote / hybrid / on-site** flag (schema.org `jobLocationType: TELECOMMUTE`) | 2,3, scrapers | **in-model** | `workplace` |
| 13 | **Date posted / apply-by (validThrough)** | 3, scrapers | **in-model** | `posted_date` / `apply_by`, from labelled lines only |
| 14 | **Typed JSON output** ready to store | 1,2 | **in-model** | `format = json` |
| 15 | **schema.org JobPosting / JSON-LD output** | 3 | **in-model** | `format = jsonld` (a differentiator — none of the parsers scanned emit ready-to-paste JSON-LD) |
| 16 | Custom/extendable skill taxonomy | 1 (org taxonomy) | **in-model** (narrow form) | `extra_skills` param — comma-separated keywords added to the built-in vocabulary |
| 17 | Cap on returned skills so output stays readable | scrapers (field caps) | **in-model** | `max_skills` (1–200, default 40) |
| 18 | Human-readable summary view alongside raw JSON | 1 (UI), scrapers (table view) | **in-model** | `format = summary` (default) |

## Defaults / UX controls observed and adopted

- **Paste-a-posting textarea as the primary input** (all UI-bearing references). → `multiline = true` on `input`, with a realistic placeholder.
- **Preset/sample buttons** so the tool shows output before the user types (common on parser demos). → three `[[example]]` chips (labelled posting, prose posting, hourly-contract posting).
- **Structured field cards / labelled output** rather than a raw blob by default. → `summary` is the default format; JSON and JSON-LD are opt-in.
- **Stated field list up front** so users know what they'll get. → the "What it pulls out" section of the page copy.

## Out-of-model (considered, not built) — gizza is browser-local wasm, no account, no server

| Feature | Seen in | Why out of model |
|---|---|---|
| ML/NLP taxonomy mapping (`Sr. SWE II` → `Senior Software Engineer`, ESCO/O*NET occupation codes) | 1,2 | Needs a trained model + a licensed occupation taxonomy; gizza is pure Rust + deterministic heuristics |
| Geocoding a location to lat/long + a canonical place record | 2 | Needs a geocoding service (network + API key) |
| Fetching a posting by URL / scraping a job board | Apify, Browse AI scrapers | Needs a server-side fetcher, per-board selectors, and login-walled pages; this tool parses **pasted text** only |
| Bulk parsing thousands of postings in parallel; stored searchable database | 1,2 | Server-side batch + storage |
| Candidate↔job matching / search-and-match scoring | 1 | Needs a resume corpus + a matching model (see the separate `resume-to-json` block for the candidate side) |
| Confidence scores per extracted field | 1 | Meaningful only for a probabilistic extractor; a heuristic parser would be inventing them |
| Industry / job-category classification | 2 | Needs a classification model + taxonomy; a keyword guess here would be wrong often enough to mislead |

## Honest positioning

This is a **deterministic heuristic parser**, not an ML one: it reads labelled lines
(`Title:`, `Company:`, `Salary:` …) first and falls back to layout + keyword heuristics. It will
be very accurate on structured postings and best-effort on free prose — the page says so, and
every field is optional in the output rather than guessed into existence. That's the honest
trade for running entirely in the browser with no upload, no account, and no per-parse cost.
