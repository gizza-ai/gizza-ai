# svg-security-linter — competitor analysis (2026-08-18)

Scan run BEFORE implementing the tool, per `/create-next-tool` step 3 + `/improve-tool` Phase 2.
Everything below is **paraphrased**; no competitor copy, branding, or trademarks are reproduced.

Search: "SVG security scanner online check SVG for XSS script tags event handlers sanitize"
(WebSearch, 2026-08-18). The result set was a mix of online scanners, sanitizer libraries, and
audit guides. Three reachable, real references were skimmed; a fourth (an in-browser SVG
sanitizer at opensvg.dev) returned HTTP 403 to the fetcher and was replaced by the audit guide
below, per the "replace an unreachable competitor" rule.

## Profiles

### 1. SVG Security Scanner (svgscanner.com) — online scanner

```json
{
  "name": "SVG Security Scanner", "url": "https://svgscanner.com/",
  "features": [
    "flags embedded script elements",
    "flags event-handler attributes",
    "flags references to external/remote resources",
    "flags attribute patterns it considers suspicious",
    "structure view: root element, attributes, child hierarchy",
    "spec-conformance notes, deprecated-feature notes, optimization hints, complexity/size metrics"
  ],
  "params_options": [],
  "input_formats": ["SVG file via drag-and-drop or file picker"],
  "output_formats": ["on-page report", "JSON export", "CSV export"],
  "output_quality": "findings list plus a structural tree; no documented severity ranking",
  "ux_patterns": ["drag-and-drop upload", "downloadable/shareable report"],
  "seo_copy_angles": ["is my SVG safe to upload", "SVG structure inspection", "SVG vulnerability check"],
  "limits": ["no stated maximum file size"],
  "free_vs_paid": "free, no documented gating"
}
```

### 2. SVG audit checklist (svgmaker.io guide) — the de-facto rule taxonomy

```json
{
  "name": "How to check if an SVG file is safe (guide)",
  "url": "https://svgmaker.io/blogs/how-to-check-if-an-svg-file-is-safe-detect-scripts-malware-and-xss",
  "features": [
    "high priority: script elements, on* event attributes, javascript: URLs, foreignObject",
    "medium priority: http(s) external references, protocol-relative // references, iframe/object/embed, CSS @import and remote font URLs, unexpected data: URLs",
    "lower priority: DOCTYPE/entity declarations (XML parser abuse), unknown namespaces, heavy obfuscation / oversized path data"
  ],
  "params_options": [],
  "input_formats": ["read the file as plain text before rendering"],
  "output_formats": ["manual checklist"],
  "output_quality": "three-tier priority ranking is the useful part",
  "ux_patterns": ["ranked checklist", "inspect-then-sanitize-then-re-scan workflow"],
  "seo_copy_angles": ["SVG upload XSS", "detect scripts in SVG", "safe SVG checklist"],
  "limits": ["guide only — no tool, nothing automated"],
  "free_vs_paid": "free article"
}
```

### 3. svg-sanitizer (darylldoyle, PHP library) — allowlist sanitizer used by CMS uploads

```json
{
  "name": "svg-sanitizer", "url": "https://github.com/darylldoyle/svg-sanitizer",
  "features": [
    "allowlist model: caller supplies permitted tags and permitted attributes",
    "optional removal of attributes that reference remote files (off by default)",
    "optional minification",
    "returns false on malformed XML rather than emitting partial output",
    "exposes the XML parse issues it hit as a retrievable list"
  ],
  "params_options": [
    {"name": "removeRemoteReferences", "type": "boolean", "default": "false", "range": "on/off"},
    {"name": "minify", "type": "boolean", "default": "false", "range": "on/off"},
    {"name": "allowed tags / allowed attributes", "type": "list", "default": "library default allowlist", "range": "caller-defined"}
  ],
  "input_formats": ["SVG/XML string"],
  "output_formats": ["sanitized SVG string, or false"],
  "output_quality": "removal-oriented; the issue log is a secondary diagnostic, not a ranked report",
  "ux_patterns": ["library API, no UI", "treats external references as a separate opt-in policy"],
  "seo_copy_angles": ["sanitize SVG uploads", "WordPress/TYPO3 SVG upload safety"],
  "limits": ["malformed XML is rejected outright"],
  "free_vs_paid": "open source"
}
```

## Table stakes → in-model / out-of-model

| Table stake (≥1 competitor) | Verdict | Where it lands |
| --- | --- | --- |
| Detect `<script>` elements | in-model | rule `SCRIPT` (high) |
| Detect `on*` event-handler attributes | in-model | rule `EVENT-HANDLER` (high) |
| Detect `javascript:` / `vbscript:` URLs | in-model | rule `JS-URL` (high), incl. entity-obfuscated forms |
| Detect `<foreignObject>` | in-model | rule `FOREIGN-OBJECT` (high) |
| Detect embedded HTML/active elements (iframe/object/embed/base/link/meta/audio/video) | in-model | rule `EMBEDDED-HTML` (high) |
| Detect external http(s) and protocol-relative references | in-model | rule `EXTERNAL-REF` (medium) |
| Make external references a policy toggle (mirrors `removeRemoteReferences`) | in-model | `allow_external` boolean, default off |
| Detect CSS `@import` / remote `url(...)` / `expression(...)` | in-model | rule `CSS-IMPORT` (medium) |
| Detect unexpected `data:` URLs | in-model | rule `DATA-URI`, escalated to high for `text/html` / `image/svg+xml` |
| Detect DOCTYPE / ENTITY declarations (XXE, entity expansion) | in-model | rule `DOCTYPE-ENTITY`, high when it names an external SYSTEM/PUBLIC id |
| Detect unknown namespaces | in-model | rule `UNKNOWN-NS` (low) |
| Three-tier severity ranking | in-model | high / medium / low + a `min_severity` filter |
| JSON export for programmatic use | in-model | `format=json` |
| CSV export | in-model | `format=csv` |
| Structured findings (element, attribute, line) | in-model | every finding carries line, element, attribute, snippet |
| Per-rule suppression after review | in-model (gizza idiom, matches the library's allowlist intent) | `ignore` rule-code list |
| Drag-and-drop file upload | out-of-model | pure-Rust pages take pasted text; binary/file inputs exist only for the ffmpeg/model runtimes |
| Structure tree / attribute browser | out-of-model for this tool | a different tool's job; would double the output for no security signal |
| Optimization + deprecated-feature hints | out-of-model for this tool | already shipped as `blocks/svg-optimize` |
| Emit a sanitized SVG | considered, rejected | a blocklist "sanitized" output gives false assurance; competitors that sanitize use an allowlist parser with a real DOM. Reporting honestly beats a half-clean file. `blocks/html-sanitizer` covers allowlist HTML cleaning |
| Malware scanning of embedded binaries | out-of-model | needs signature databases / a backend |
| Rendering preview of the SVG | considered, rejected | rendering untrusted SVG in the page is the exact risk the tool warns about |

## Rules added beyond the competitor set

Real SVG XSS vectors none of the three call out explicitly, added because they are cheap and
high-signal:

- `ANIMATE-HREF` — SMIL `<animate>`/`<set>`/`<animateTransform>`/`<animateMotion>` whose
  `attributeName` targets `href`/`xlink:href`/`style`/an `on*` handler. This is a known
  script-injection route that survives naive "strip `<script>` and `on*`" filters.
- `HANDLER` — the SVG 1.2 `<handler>` element and `ev:*` event attributes.
- `XML-STYLESHEET` — an `<?xml-stylesheet href="…"?>` processing instruction, which pulls a
  stylesheet before any element-level filtering sees it.
- `ANCHOR-TARGET` — `<a target="_blank">` without `rel="noopener"` (low; reverse-tabnabbing).

## Decisions

- Output defaults to a ranked text report; `json` and `csv` cover the export angle.
- A `verdict` (`unsafe` / `review` / `clean`) is computed from the findings that survive
  `ignore` + `allow_external`, **before** the `min_severity` display filter, so raising
  `min_severity` can never turn an unsafe file into a clean-looking report.
- Input cap: 1,000,000 bytes, stated on the page (no competitor states one; silence there is a
  gap, not a model to copy).
