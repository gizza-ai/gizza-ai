# csv-pii-redactor — competitor analysis (2026-07-07)

Column-scoped PII redaction over chosen CSV columns: mask, salted-hash, or uniform-redact
the values in named/indexed columns for safe sharing. Distinct from `redact-pii` (free-TEXT
regex masking) and `pii-tokenize` (free-TEXT format-preserving pseudonyms) — those operate on
prose, not on chosen tabular columns, and neither offers a salted-hash-per-column mode.

## Competitors scanned

1. **csvtools.com — Anonymize CSV Columns** (client-side, browser). Per-column method dropdown.
   Methods: Omit (drop column), Redact (uniform marker), Hash (short, 8-char code), Hash (full,
   64-char code), Fake Name, Fake Contact. Consistent mapping: same value → same replacement.
2. **terrific.tools — Data Anonymizer** (client-side; returned 403 to the fetcher, replaced with
   SplitForge — details below preserved from the search summary): redact / mask / replace of
   emails, phones, IPs, SSNs per selected column; custom redaction string; header toggle;
   delimiter/quote/comment config.
3. **SplitForge — CSV Data Masking** (client-side, "files never uploaded"): 8 techniques grouped
   as substitution, redaction, pseudonymization, hashing, tokenization; configure masking per
   column; upload → adjust per column → download.

(Reference on technique choice: IRI "PII masking in CSV files" — hashing for birthdates,
string-redaction for IDs/licenses, date-shifting for dates; pick the function per field.)

## Table-stakes matrix (each ends in the descriptor OR the out-of-model list)

| Capability | In/Out | How ours covers it |
|---|---|---|
| Pick which columns to redact | IN | `columns` (names when header=true, else 1-based indices) |
| Header row toggle | IN | `header` (default true) |
| Delimiter config (`,` tab `;` `|`) | IN | `delimiter` |
| Redact = uniform marker | IN | `mode=redact` + editable `label` (default `[REDACTED]`) |
| Mask = char replacement, keeps length | IN | `mode=mask` + `mask_char` (default `*`) |
| Partial mask — keep last N visible | IN | `keep_last` (slider) — e.g. card → `****1234` |
| Hash = deterministic per-value code | IN | `mode=hash` (salted SHA-256, hex) |
| Salt the hash (anti rainbow-table) | IN | `salt` — same salt+value → same code (linkable) |
| Hash length short(8) / full(64) | IN | `hash_length` slider (4–64, default 8) |
| Consistent mapping (same in → same out) | IN | all three modes are deterministic |
| Paste / file / URL input | IN (paste) | page textarea + CLI `data=`; large-file upload is out of scope for a pure text tool |
| Presets | IN | `[[example]]` chips (mask names, salted-hash emails, keep-last-4 card) |
| Editable mask char / redaction string | IN | `mask_char`, `label` |
| Per-column *different* method in one pass | OUT (UI) | one mode per run; run once per method group. A single-descriptor page can't host a dynamic per-column method grid. Documented on the page. |
| Omit / drop a column | OUT (dup) | that is `csv-reorder-columns` — cross-linked, not duplicated here |
| Fake/synthetic data (realistic names, emails) | OUT | needs a faker dataset; a different tool category (synthetic-data generation), not redaction |
| Date shifting / blurring | OUT | a different transform (date arithmetic per row); candidate for its own tool |
| Auto-detect which columns are PII | OUT | this tool is column-explicit; free-text detection is `redact-pii`. Cross-linked. |

## UX controls matched

- Sliders for `keep_last` and `hash_length` (`kind = "slider"`).
- `<select>` for `mode` via `Param::enumv`.
- Preset chips (`[[example]]`) standing in for competitors' method presets.
- Editable mask character and redaction label text fields.
- Multiline CSV textarea (`multiline = true`).

## Decisions

- Build it: genuinely distinct (column-scoped + salted-hash), confirmed against `redact-pii` /
  `pii-tokenize` cores.
- Salt is prepended: `hex(SHA256(salt || value))` truncated to `hash_length`. Deterministic, so
  values stay joinable across files hashed with the same salt; the salt defeats naive
  rainbow-table reversal.
- Out-of-model items are listed, never built. No competitor copy/branding/trademark reproduced.
