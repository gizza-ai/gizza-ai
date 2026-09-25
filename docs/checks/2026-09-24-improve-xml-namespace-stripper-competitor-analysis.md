# xml-namespace-stripper — competitor analysis (2026-09-24)

Scan run BEFORE implementation (new tool, so the "improve" pass folds into the build).
Top 3 **reachable** competitor surfaces, plus the canonical XSLT recipe that every
namespace-stripping guide reproduces. **All notes are paraphrased — no competitor copy,
branding, or trademarks are reproduced or reused.**

## Profiles

### 1. MyXML online XML utilities (`myxml.in/xml-utils.html`)
- **Features:** a menu of XML utilities in one page — namespace removal, namespace
  *optimization* (hoist repeated declarations to the root), prettify/indent, alphabetical
  element sort, sort repeating elements by a child value, drop `xsi:nil="true"` attributes,
  strip XSD annotations, wrap in a SOAP 1.1/1.2 envelope, XSD/JSON/DDL conversions, treeview,
  XPath extraction.
- **Namespace options:** (a) remove all namespaces — declarations *and* prefixes; (b) optimize
  prefixes; (c) re-emit with the namespace as a default (unprefixed) declaration. Demonstrated
  on `<ns:root xmlns:ns="…">` → `<root>`.
- **Input/output:** paste-only textarea, button per action, result in an output pane below.
- **UX patterns:** one page, many verbs; no upload; namespace-sort needs four hand-typed
  parameters (parent element, repeating element, sort child, element-vs-attribute).
- **Limits stated:** none.
- **Free vs paid:** free, no account.

### 2. EasyProTools XML tag stripper (`easyprotools.com/text/xml-tag-stripper/`)
- **Features:** five strip modes (all tags · a named tag list · keep-only a named list ·
  attributes only · **namespace prefixes**) plus independent toggles for: strip the XML prolog,
  strip comments, unwrap CDATA, strip DOCTYPE/DTD, strip processing instructions, decode
  entities, **strip namespace declarations**, **strip schema references**. Attributes can be
  removed, kept, or extracted separately. Nine extraction output shapes (plain text, CSV, JSON,
  tag+value pairs, …), XPath query box with preset buttons, tree view, bulk/batch runs,
  search-and-replace, pretty-print, line-ending choice.
- **Namespace options:** prefix stripping and declaration stripping are **two separate
  switches**, and "strip schema references" is a third.
- **Input/output:** paste · fetch by URL · single upload · batch upload. Copy and download.
- **Limits stated:** 15 MB per uploaded file, 10 MB for URL-fetched content.
- **Processing:** server-side (PHP) parse + validation with error reporting.
- **FAQ angles:** which file types are accepted, how validation works, how the strip modes
  differ, XPath usage, CDATA handling, URL fetching, extraction formats, privacy, pretty-print,
  search-and-replace.

### 3. `hxxmlns` (W3C HTML-XML-utils, `w3.org/Tools/HTML-XML-utils/man1/hxxmlns.html`)
- **Features:** CLI filter. Rewrites every element and attribute name to a global
  `{namespace-URI}local` form and **deletes every `xmlns…` declaration attribute**.
- **Params:** `-d` also drops comments, processing instructions and the DOCTYPE; otherwise those
  are preserved. Input file argument, else stdin.
- **Caveat it documents:** unprefixed attributes are reported with an *empty* namespace
  (`{}name`) rather than inheriting the element's default namespace — the correct XML-namespace
  rule, and a trap for naive rewriters.
- **Limits:** none (stream filter).

### 4. Canonical XSLT recipe (IBM / Microsoft / TEI / Roy Tutorials write-ups)
- Identity transform with three templates: elements re-created as `local-name()`, attributes
  re-created as `local-name()`, comments/PIs/text copied through.
- **Documented caveats, repeated by every source:** (a) two elements or attributes whose local
  names collide (the classic example is a document mixing two vocabularies that both define
  `title`) become indistinguishable — data loss or ambiguity; (b) an element carrying two
  attributes with the same local name under different prefixes is an outright **name clash**;
  (c) stripping namespaces is lossy and shouldn't be done when the namespaces carry meaning.

## Table stakes (must exist, or be explicitly out-of-model)

| # | Table stake | Seen in | Verdict |
| - | ----------- | ------- | ------- |
| 1 | Remove `xmlns` and `xmlns:prefix` declarations | all 4 | **in-model** — `mode=all`/`declarations` |
| 2 | Remove element prefixes (`ns:root` → `root`) | all 4 | **in-model** — `mode=all`/`prefixes` |
| 3 | Remove attribute prefixes (`xsi:type` → `type`) | all 4 | **in-model** — same |
| 4 | Declarations and prefixes as *independent* choices | EasyProTools, hxxmlns | **in-model** — `mode` enum covers all 3 useful combinations |
| 5 | Preserve comments, PIs, CDATA, DOCTYPE, prolog, text | hxxmlns (default), XSLT | **in-model** — quick-xml round-trips every event; default output is byte-identical apart from the namespace edits |
| 6 | Strip dangling schema references (`xsi:schemaLocation`) | EasyProTools, MyXML (`xsi:nil`) | **in-model** — `remove_schema_hints` (default on), matched by resolved namespace URI, not by prefix spelling |
| 7 | Handle attribute local-name clashes | XSLT caveat (b), TEI warning | **in-model, and a differentiator** — `conflicts=rename\|first\|error`; competitors emit broken or silently lossy output here |
| 8 | Keep selected prefixes | MyXML (partial: optimize/default-ns) | **in-model** — `keep` list, keeps the prefix *and* its declaration |
| 9 | Pretty-print / minify the result | MyXML, EasyProTools | **in-model** — `format=preserve\|pretty\|minify` + `indent` |
| 10 | Well-formedness error with a position | EasyProTools (server-side) | **in-model** — byte-position error from the parser, locally |
| 11 | Copy / download the result | MyXML, EasyProTools | **in-model, already platform** — the generator gives every text tool Copy + Download + Reset |
| 12 | Report what was removed | none (no competitor surfaces this) | **in-model, differentiator** — `output=report` CSV listing every removed declaration with its namespace URI plus per-category counts |
| 13 | Reserved `xml:` prefix handling | hxxmlns caveat / XSLT does *not* handle it | **in-model, differentiator** — `xml:lang`/`xml:space`/`xml:id` are preserved (the `xml` prefix is bound by spec and never declared); the naive `local-name()` recipe destroys them |

## In-model decisions (built)

- `mode` = `all` (default) · `prefixes` · `declarations` — table stakes 1, 2, 3, 4.
- `keep` — comma-separated prefixes to leave alone, with their declarations; the token `xmlns`
  keeps the default (unprefixed) declaration. Table stake 8.
- `conflicts` = `rename` (default, `a:x` → `a_x`, nothing lost) · `first` (keep the first, drop
  later collisions) · `error`. Table stake 7.
- `remove_schema_hints` (default true) — drops `schemaLocation` / `noNamespaceSchemaLocation`
  whose prefix actually resolves to the XML-Schema-instance namespace, because after stripping
  they reference namespaces the document no longer declares. Table stake 6.
- `format` = `preserve` (default, byte-faithful) · `pretty` (+ `indent`) · `minify`. Table stake 9.
- `output` = `xml` (default) · `report`. Table stake 12.
- Preserved unconditionally: comments, processing instructions, CDATA, DOCTYPE, the XML
  declaration, text nodes, entity references, attribute values, and the reserved `xml:` prefix.
  Table stakes 5, 13.
- Errors name the byte position (malformed XML), the expected values (bad enum), the cap
  (oversized input), and the clashing pair (`conflicts=error`). Table stake 10.

## Considered, rejected (in-model but declined)

- **Namespace "optimization"** (hoisting repeated declarations to the root while keeping
  prefixes) — that is a namespace *rewriter*, not a stripper; it belongs on `xml-formatter`
  rather than bloating this tool's schema with a fourth mode whose output still has prefixes.
- **Dropping `xsi:nil="true"`** — `nil` carries document meaning (it marks an element as
  explicitly null); removing it changes data rather than removing namespace plumbing.
  `schemaLocation`/`noNamespaceSchemaLocation` are pure schema *references*, which is why those
  are in and `nil` is out.
- **Alphabetical element / value sorting, SOAP envelope wrapping, XSD annotation stripping** —
  unrelated verbs bundled into competitors' one-page utility menus; the toolkit already covers
  the adjacent jobs with focused tools.

## Out-of-model (needs a server, an account, or a surface gizza doesn't have)

- URL fetching of the XML to strip — needs network from the page; composing with the existing
  `web-fetch` block covers it.
- File upload / multi-file batch runs (their 15 MB per-file, 10 MB per-URL caps) — the pure page
  surface takes pasted text, not uploads.
- Server-side validation and error reporting — this tool parses locally in wasm; nothing is
  uploaded, which is the positioning advantage, not a gap.
- Interactive tree visualization, an XPath query box, search-and-replace, XSD/JSON/DDL
  conversion, nine extraction output shapes — separate jobs already served by focused blocks
  (`xpath-query`, `xml-to-json`, `xml-to-csv`, `xml-formatter`, `xml-diff`).

> Original work only — no competitor copy, branding, or trademarks copied.
