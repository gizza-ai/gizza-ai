## About this tool

This OPML converter transforms **OPML subscription files** to and from **JSON**
and **CSV** in any direction, right in your browser. The conversion runs locally
in WebAssembly — your file is **never uploaded** to a server, so a private
subscription export stays on your machine.

**OPML** (Outline Processor Markup Language) is the XML format that podcast apps
and RSS readers use to export and import your subscriptions. An OPML file is a
tree of `<outline>` elements: each feed carries attributes like `text`, `type`,
`xmlUrl`, and `htmlUrl`, and feeds can be nested inside folder outlines
(categories). This tool models that whole tree so it can round-trip cleanly.

### Conversions

- **OPML → JSON** and **JSON → OPML** — a faithful round-trip of the entire
  outline tree. Each `<outline>` becomes a JSON object of its attributes plus a
  nested `"outlines"` array of its children, so folders and feed metadata
  survive the trip in both directions.
- **OPML → CSV** and **CSV → OPML** — CSV is the flat *feed list*: one row per
  feed, with the common columns (`text`, `type`, `xmlUrl`, `htmlUrl`) plus a
  `category` column that records the folder path (joined with ` / `). Because
  the path is preserved, converting the CSV back to OPML or JSON rebuilds the
  original nested folders.
- **JSON ⇄ CSV** — go straight between the two data formats the same way.

### Options

- **From / To** — pick the source and target format (OPML, JSON, or CSV).
- **Pretty-print output** — indent the OPML or JSON output for readability
  (on by default). Turn it off for compact, single-line output. This has no
  effect on CSV.

### Notes

- **Folders round-trip.** Nested categories become a slash-joined `category`
  column in CSV and a nested `"outlines"` array in JSON, then rebuild back into
  nested `<outline>` folders when you convert to OPML.
- **Attribute order is preserved**, so re-exported OPML keeps feed attributes
  (`text`, `type`, `xmlUrl`, …) in the order they were authored.
- **Spreadsheet-friendly.** Export your podcast or RSS subscriptions to CSV to
  edit them in a spreadsheet, then convert back to OPML to re-import into your
  app.
- **Nothing is uploaded** — the conversion happens entirely in your browser, so
  it works offline and keeps your subscription list private.
