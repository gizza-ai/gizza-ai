# fake-data-generator — competitor analysis (2026-06-22)

New tool built from the backlog (`Generates rows of fake test records (name,
email, phone, address) as CSV or JSON`), then improved against the top fake/mock
data generators. Pure-Rust, browser-local (no network, no ML, no data files).

## Surfaces verified

- **Chat block**: `wafer build` validates + instantiates (340 KiB). Seed=0 path
  derives a varying seed from the wall clock; non-zero seed is reproducible.
- **CLI** (`gizza tool fake-data-generator …`): csv / json / sql / xml, field
  subset selection, header toggle, seed reproducibility, unknown-field/format
  errors — all verified.
- **Page** (`/tools/fake-data-generator/`): Playwright covers CSV, JSON, SQL, and
  the CSV header checkbox (4 tests, all pass).
- 20 unit/drift tests pass (Luhn validity, UUID v4 shape, format/data parity per
  seed, XML/CSV/SQL escaping).

## Competitors surveyed

- **Mockaroo** — industry standard (free ≤1000 rows). ~75–170 field types (name,
  email, phone, address, IP v4/v6, MAC, UUID, credit card, lat/long, color, …);
  formats CSV, JSON, SQL, XML, Excel, tab-delimited; per-field null %, header/BOM
  toggles, locale, schema-from-sample.
- **Faker.js (fakerjs.dev)** — code library, 28 modules (person, location,
  internet, finance, lorem, color, …); locales; seed; regex strings.
- **generatedata.com** — open-source engine, ~30 types, 12 export formats (CSV,
  JSON, SQL, XML, HTML, LDIF, + code arrays); per-column naming, country datasets,
  NULL ratio.
- **JSON Generator** — JSON-only template engine; index/firstName/email/phone/
  lorem/guid/objectId/bool; repeat ranges; custom JS.
- **Bogus (.NET)** — Faker port; same module coverage; deterministic via seed.

## Gaps closed (in-model, pure Rust)

| Gap (competitor parity) | Implementation |
|---|---|
| SQL output format | `INSERT INTO <table> (...) VALUES (...);`, numeric/boolean columns unquoted, `'` doubled in string literals, configurable table name |
| XML output format | `<records><record><col>val</col></record></records>`, entity-escaped |
| CSV header toggle | `header` boolean (default true), renders as page checkbox |
| UUID v4 field | 16 seeded RNG bytes with version/variant nibbles set |
| IPv4 / IPv6 / MAC fields | formatted random byte/group tuples |
| Color (hex) field | `#rrggbb` |
| Boolean field | seeded coin flip |
| Latitude / longitude fields | scaled RNG floats in valid ranges |
| Lorem `sentence` field | embedded static word pool, capitalized + period |
| Domain field | company prefix + embedded TLD list |
| Credit-card field | 16 digits with a valid Luhn check digit (test placeholder) |
| Reproducibility seed | already present; non-zero seed → identical dataset |

Field count grew from 15 → 26 columns; formats from 2 → 4.

## Out-of-model (NOT built — needs network / data files / locale corpora)

- Per-field null/blank percentage with weighting — partial fit, deferred (could
  be a future `null_pct` param; left out to keep the schema lean this pass).
- Locale-specific name/address corpora (de/fr/ja datasets), real country
  datasets, schema-from-sample, AI/formula columns — require bundled datasets or
  a model, outside the pure browser-local model.
- Excel / LDIF / code-array exports — niche; CSV/JSON/SQL/XML cover the common
  cases.

## Notes

All output is synthetic placeholder data, not real people; credit-card numbers
are Luhn-valid test placeholders, never real accounts. No competitor copy,
branding, or trademarks were used.
