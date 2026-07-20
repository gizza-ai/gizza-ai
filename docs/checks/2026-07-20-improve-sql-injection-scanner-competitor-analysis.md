# sql-injection-scanner — competitor analysis (2026-07-20)

Function of our tool: a **static code scanner** — paste a code snippet, get the lines where a
SQL query is built by string concatenation/interpolation/format instead of parameterized. This
is the "is this CODE vulnerable" job (like a security linter), NOT the "is this INPUT STRING an
attack payload" job and NOT the "scan a live URL" job.

## Competitors surveyed (paraphrased; no copy/branding reproduced)

1. **consolelog.tools — SQL Injection Tester (Analyze mode).** Closest match. Inspects a SQL
   query or app-code snippet and reports how the query is constructed: flags string
   concatenation and template interpolation (`+ "…"`, `${…}`, f-strings, `%s`, `.format()`),
   unbalanced quotes, comment sequences, `OR 1=1`, `UNION SELECT`, time-based markers
   (`SLEEP`, `WAITFOR DELAY`, `BENCHMARK`, `pg_sleep`), stacked queries, hex encoding, dynamic
   table/column names, and absence of placeholders. Single textarea + an analyze button.

2. **Bandit B608 (`hardcoded_sql_expressions`).** The canonical Python static rule. Flags
   SQL-like strings built via `%` interpolation, `+` concatenation, `.format()`, f-strings, and
   (since 1.7.7) `str.replace()`. Tiered confidence: LOW for an isolated built SQL string,
   MEDIUM when it lands in a DBAPI `execute()/executemany()`. Reports MEDIUM severity, uses a
   keyword heuristic to decide a string "looks like SQL".

3. **Elysia Tools — SQL Injection Detector.** Textarea + a Quick/Full scan dropdown. Detects
   tautologies (`' OR '1'='1'`), UNION signatures, stacked queries, time-based blind markers,
   and encoding obfuscation. Purely static text matching; risk classifications ("high-risk"),
   example use cases, but this one leans toward *payload* detection rather than code review.

(Also seen: toolsnip payload tester — a single input + preset attack strings; and the URL-based
dynamic scanners Acunetix / Invicti / Intruder / pentest-tools — those crawl a live site and
send payloads, a fundamentally different, out-of-model job.)

## Table-stakes → decision

| Capability / control | In competitors | Our decision |
|---|---|---|
| Paste a code snippet (multiline) | all | IN — `code` multiline field |
| Detect string concatenation (`+`, PHP `.`) | consolelog, Bandit | IN — `SQLI-CONCAT` |
| Detect interpolation (f-string, `${…}`, `$"…"`, `#{…}`) | consolelog, Bandit | IN — `SQLI-INTERP` |
| Detect format/printf (`.format`, `%`, sprintf, String.Format) | consolelog, Bandit | IN — `SQLI-FORMAT` |
| Warn on execute(variable) / DBAPI sink | Bandit (MEDIUM) | IN — `SQLI-EXEC-VAR`, medium |
| Recognize parameterized calls as safe | Bandit (confidence tiers) | IN — `execute("…", params)` → no finding |
| Line numbers per finding | Bandit, consolelog | IN — line + column |
| Severity / risk classification | all | IN — high / medium |
| Rule id | Bandit (B608) | IN — `SQLI-CONCAT/INTERP/FORMAT/EXEC-VAR` |
| Remediation guidance / fix example | consolelog, Bandit docs | IN — text footer + per-language example |
| Language selection | (implicit; Bandit is Python-only) | IN — `language` enum scopes patterns + tailors fix |
| Quick vs Full scan / severity filter | Elysia (dropdown), consolelog | IN — `min_severity` (all / high) |
| Structured/exportable output | S4E (PDF export) | IN — `format` = text / json (PDF export out) |
| Preset example inputs | toolsnip (preset payloads), consolelog | IN — 5 `[[example]]` chips |
| **Payload/attack-string detection** (`' OR 1=1`, UNION, SLEEP) | consolelog, Elysia, toolsnip | OUT-OF-SCOPE — that is the inverse job (is this *input* malicious), a distinct tool; noted on the page as "not what this does" |
| **Live-URL / dynamic scanning** (crawl + send payloads) | Acunetix, Invicti, Intruder, pentest-tools, S4E | OUT-OF-MODEL — needs a running target + network fuzzing; gizza tools are pure/local |
| **PDF / report export** | S4E | OUT — JSON output covers structured export; no PDF renderer here |
| **AI-assisted analysis** | AI SQL Check | OUT-OF-MODEL — no model in a pure block |

## UX controls matched
- Multiline paste field (all competitors have a textarea).
- Language `<select>` with friendly labels.
- Scan-depth `<select>` (`min_severity`) — the "Quick/Full" analogue.
- Output-format `<select>` (text / JSON).
- 5 one-click example chips (concat, f-string, PHP concat, JS template literal, and a
  safe-parameterized "no findings" case) — the preset-input pattern competitors ship.

## Notes
- No competitor copy, branding, or trademark text was reproduced. Fix examples and messages are
  our own wording.
- The payload-detection function (`' OR 1=1`, UNION, time-based) is deliberately excluded: it is
  a different tool (input-classifier), and folding it in would blur what "a finding" means
  (vulnerable code vs. a malicious input). Stated explicitly on the page.
