# yaml-formatter — competitor analysis (2026-07-30)

Snapshot for the new-tool + improve pass. Five real, reachable YAML
formatter/beautifier tools surveyed via web search. All notes are **paraphrased**
— no competitor copy, branding, or trademarks reproduced.

## Competitor profiles (top 5)

### 1. JSONFormatter.org — YAML Formatter (`jsonformatter.org/yaml-formatter`)
- **Features:** reindents messy YAML into a valid layout; doubles as a
  viewer/lightweight validator; exports parsed data to JSON and CSV; shareable
  link; optional server-side save with an expiry window.
- **Params/options:** an indentation selector (not clearly exposed); no explicit
  sort / flow / quote toggles surfaced.
- **Input/output:** paste, file upload, or load-from-URL → formatted YAML, JSON,
  CSV.
- **UX:** upload / URL-load, download, share buttons, sample reference.
- **Limits / pricing:** no stated size cap; free, ad-supported.

### 2. Online YAML Tools — Prettify YAML (`onlineyamltools.com/prettify-yaml`)
- **Features:** rewrites cramped/inline YAML into an expanded document; sits in a
  suite with separate sort-by-key/value, minify, and convert utilities.
- **Params/options:** spaces-per-depth-level number (default 2).
- **Input/output:** paste or file import → prettified YAML.
- **UX:** live dual-pane preview, import/download/copy, clickable worked examples
  that preload options, URL query-param support (input + indent width).
- **Limits / pricing:** no stated cap; free, no ads, fully in-browser.

### 3. HCODX — YAML Formatter (`hcodx.com/tools/yaml-formatter`)
- **Features (richest):** width choice + compact/expanded toggle; recursive
  alphabetical key sort (off by default); beautify (block) vs minify (flow);
  syntax validation with line-numbered errors; option to keep anchor/alias
  references instead of inlining.
- **Params/options:** indentation `2 / 4 / tab`; alphabetize-keys toggle; style
  `beautify (block) / minify (flow)`; keep-anchors toggle.
- **Input/output:** paste (js-yaml 4.x, YAML 1.2) → YAML (+ JSON via linked
  converter).
- **UX:** live dual-pane editor, format-on-keystroke, copy/download, size + line
  metrics, before/after example.
- **Limits / pricing:** no stated cap; free, in-browser.

### 4. Teleport — YAML Beautifier (`goteleport.com/resources/tools/yaml-beautifier/`)
- **Features:** indentation cleanup + key/value alignment; aims to preserve
  comments and blank lines; best-practice guidance on line length.
- **Params/options:** no concrete controls exposed (guidance recommends 2 or 4
  spaces).
- **Input/output:** paste → beautified YAML.
- **UX:** embedded widget with explainer copy, cross-links to sibling utilities.
- **Limits / pricing:** free vendor-hosted lead-gen utility.

### 5. Formatter.org — YAML Formatter (`formatter.org/yaml-formatter`)
- **Features:** auto-adjusts indentation and alignment; strips clutter; aimed at
  config/script maintenance.
- **Params/options:** none documented.
- **Input/output:** paste → formatted YAML.
- **UX:** simple paste-and-format page with converter cross-links.
- **Limits / pricing:** free, ad-supported.

## Gap analysis vs our tool

| Dimension | Competitor does | Our status | Verdict |
| --- | --- | --- | --- |
| Indent width | 2/4/tab (HCODX), N spaces (onlineyamltools) | `indent` 1–8 spaces | **In-model, built.** Tabs deliberately excluded — YAML forbids tab indentation, so a "tab" option produces invalid YAML. |
| Key sorting | Recursive A→Z (HCODX, onlineyamltools sibling) | `sort_keys` preserve/asc/desc | **In-model, built** — added desc as a bonus. |
| Block vs flow | Beautify/minify toggle (HCODX) | `style` block/flow | **In-model, built.** |
| Validation / errors | Line-numbered errors (HCODX) | Parser error surfaced via `invalid YAML: <msg>` | **In-model, built** (message-level; line numbers come from serde_yml where available). |
| Multi-document `---` | Not emphasized by competitors | Supported, each doc normalized | **In-model, built** (a small edge over the field). |
| Comment / blank-line preservation | Teleport claims comment preservation; HCODX keeps top-of-file comments | Not preserved (Value-model parse) | **Considered, not built for v1** — faithful comment-preserving reformat needs a CST-preserving parser (not serde_yml). Stated clearly on the page + descriptor. |
| Keep anchors/aliases | HCODX toggle | Aliases expanded, anchors dropped | **Out-of-model for v1** — same CST limitation; documented. |
| YAML ⇄ JSON/CSV/TOML export | JSONFormatter, onlineyamltools, Formatter.org | Not this tool | **Out of scope** — gizza already ships `json-yaml-convert` / `json-yaml-converter`; this tool stays a YAML→YAML formatter and points users there. |
| Load-from-URL / file upload / share links / server save | JSONFormatter | Paste only, browser-local | **Out-of-model** — needs a backend / account; conflicts with the no-server, browser-local model. |
| Dual-pane live preview | HCODX, onlineyamltools | Single input + output panel with examples | **Considered** — the shared generator's input→output layout + example chips cover the core need without a per-tool editor. |

## Out-of-model features considered, not built
- **Comment / blank-line / anchor preservation** — requires a concrete-syntax-tree
  parser; serde_yml discards them. Documented as a limit.
- **YAML ⇄ JSON/CSV/TOML conversion** — already covered by sibling gizza tools.
- **URL/file loading, share links, cloud save** — need a server/account.
