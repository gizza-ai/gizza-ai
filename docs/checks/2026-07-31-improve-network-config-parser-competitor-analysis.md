# network-config-parser — competitor analysis (2026-07-31)

Tool: parse a hierarchical network device configuration (Cisco IOS / IOS-XE /
NX-OS, Arista EOS, Juniper Junos) into a foldable, searchable tree of sections.
Type: pure (text in → structured JSON out, no I/O). All copy below is
**paraphrased** — no competitor copy, branding, or trademarks reproduced.

## Competitor scan (top real tools)

The "network config parser" space is dominated by Python libraries rather than
polished web tools; these three are the real, reachable references.

### 1. CiscoConfParse / CiscoConfParse2 (mpenning)
- **Function:** parses "Cisco-style" indented configs and "Juniper-style"
  brace-delimited configs into linked parent/child objects.
- **Features:** multi-vendor (Arista, Cisco, Juniper, Palo Alto, F5); searches
  across any number of nesting levels (v2; v1 was parent+child only);
  find-objects / find-parents-with-a-child regex queries; audit / build / modify
  configs.
- **Input/output:** text config in; object graph / lists of matching lines; can
  emit branches as text.
- **UX:** library API (Python), not a form; regex-driven queries.
- **Limits stated:** heavily-nested vendors (Juniper/PAN/F5) need the v2 engine.

### 2. cisco_config_parser (Klevernet / arezazadeh)
- **Function:** parses Cisco IOS / IOS-XE / IOS-XR / NX-OS into objects and/or
  JSON, exposing the full config hierarchy under a config tree.
- **Features:** parent/child tree; JSON export; per-section access (interfaces,
  routing, etc.).
- **Input/output:** IOS-family text in; Python objects or JSON out.
- **UX:** library API.

### 3. shconfparser (network-tools)
- **Function:** parses the `show` outputs of Cisco and other vendors, translating
  them into tree / table / data formats.
- **Features:** breaks a config into parent/child relationships; tree, table and
  data (dict) renderings.
- **Input/output:** show-command text in; tree/table/dict out.
- **UX:** library API.

## Table-stakes distilled (params / behaviors)

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Parent/child hierarchy from **indentation** (Cisco/NX-OS/Arista) | in-model | built (`syntax=indent`) |
| Parent/child hierarchy from **braces** (Juniper Junos) | in-model | built (`syntax=brace`) |
| Auto-detect the config style | in-model | built (`syntax=auto`, default) |
| **Nested JSON tree** output | in-model | built (`output=tree`, default) |
| **Flattened command paths** (Junos `set`-style leaf paths) | in-model | built (`output=paths`) |
| **Search / filter** the tree (the "searchable" requirement) | in-model | built (`filter=` substring, keeps context) |
| Summary **stats + top-level section list** | in-model | built (`output=report`) |
| **Comment / separator** handling (`!`, `#`, `/* */`) | in-model | built (`comments=strip\|keep`) |
| Regex query language (find_objects, parents-with-child) | out-of-model | listed, not built — a mini query DSL is scope creep for a paste-and-view tool |
| Config **modify / build / diff / audit** | out-of-model | listed, not built — this tool is read-only parse+view |
| Per-vendor **semantic** normalization (interpret OSPF areas, ACLs…) | out-of-model | listed, not built — syntactic tree only |
| Table / CSV rendering of `show` output | out-of-model | listed — this tool targets *config* files, not `show` tables |
| YAML output | in-model (minor) | considered, rejected — JSON tree already covers the need; avoids schema bloat |

## Design decisions

- **Two syntaxes, one node model.** `Node { line, children }`; the root is a list
  (a config can repeat headers like two `interface` lines, so an array — not an
  object — is the correct tree shape).
- **Indent style:** nesting by leading-whitespace width (tab = 8 columns); a line
  is a child of the nearest earlier line with strictly smaller indent. Bare `!`
  are Cisco separators (always dropped); `!`-prefixed text and `#` lines are
  comments (dropped by default, kept as nodes with `comments=keep`).
- **Brace style:** a trimmed line ending in `{` opens a section (header = text
  before `{`); `}` closes; other lines are leaf statements (trailing `;`
  stripped). `#`-to-EOL and `/* … */` comments are stripped by default.
  Unbalanced braces are a clear error with the offending line number.
- **`filter`** is a case-insensitive substring: matching a section header keeps
  its whole block; matching a deep value keeps its ancestor chain — so the pruned
  tree is always valid and shows context (mirrors how the library search keeps
  parent context).
- **Defaults** chosen for paste-and-go: `syntax=auto`, `output=tree`,
  `comments=strip`, empty `filter`.

## Verification

Unit tests (happy + error per behavior), drift-guard schema test, CLI exact-output
case, Playwright page + `?config=`/`?filter=` deep-link, hygiene gate.
