# csv-to-xml — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/csv-to-xml` — convert a CSV table into XML records using the
header names as element tags. Pure-Rust (`csv`). Pure-text input → text output:
chat + CLI + a page.

## What competitors do

- **Online "CSV to XML" converters** — paste CSV, get XML. Common, but the data is
  uploaded and tag/escaping behaviour is often unclear.
- **`pandas df.to_xml()` / xmlstarlet / scripts** — local + correct, but need a
  Python/CLI environment and setup.
- **ETL/spreadsheet exports** — heavyweight for a quick conversion.

## How this tool competes / improves

1. **Runs locally + everywhere.** Pure-Rust compiled to wasm: chat, CLI, and an
   in-browser page. The CSV never leaves the device.
2. **Valid XML by construction.** Header names are **sanitized to valid XML element
   names** (invalid chars → `_`, leading digit gets a `_` prefix) and all values
   are **XML-escaped** (`&`, `<`, `>`) — so the output parses, unlike naive
   string-concatenation converters.
3. **Customisable structure.** Choose the **root** and **record** element tags;
   toggle whether the first row is the header (else `col1…`); pick the delimiter
   (`,` / tab / `;` / `|`). A real CSV parser handles quoted fields.
4. **Includes the XML declaration** so the output is a complete document.
5. **Agent-friendly.** One call to turn tabular data into XML records for a legacy
   system or an XML pipeline.

## Honest scope

- **Element-per-field records** (`<row><col>val</col></row>`) — not attribute-based
  XML, nested/hierarchical structures, or a schema/XSD.
- **CSV → XML** only (not the reverse).

## Tests

6 core unit tests: basic records with root/record/field tags; header sanitised to
a valid XML name (spaces → `_`, leading digit prefixed); values XML-escaped;
no-header uses `col1…`; tab delimiter + default tag names; and errors (empty input,
bad delimiter). Plus the block drift-guard schema test. **CLI verified** end-to-end.
**Page** verified with Playwright. `wafer build` instantiates the chat block
(322 KiB).
