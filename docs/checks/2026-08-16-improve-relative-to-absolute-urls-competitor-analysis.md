# relative-to-absolute-urls — competitor analysis (2026-08-16)

Scan run before implementing the block. One web search ("convert relative URLs to absolute
URLs in HTML online tool"), then the top real tools/libraries in the results were read
directly. Everything below is **paraphrased**; no competitor copy, branding or trademark is
reproduced, and no competitor wording was reused in the page.

## Competitors reviewed

### 1. WillMaster "Relative to Absolute URLs Converter" (PHP script, hosted page)
The only genuinely comparable *tool* (as opposed to library) in the results.

- **Inputs:** two fields — a base URL ("the URL the relative links are relative to") and a
  filename. The script is uploaded next to the HTML file it edits and rewrites that file in
  place, taking a timestamped backup first.
- **Attributes covered:** `src` and `href` only.
- **Resolution:** validates that the base looks like http/https, splits out host + path, and
  resolves `../` segments before rebuilding the absolute URL.
- **Skips:** values that already carry a scheme; fragment-only and query-only values.
- **Options:** none — explicitly "runs as-is, no configuration".
- **Limits:** requires PHP, filesystem write access and the file to sit beside the script;
  no paste-in surface, no preview, no per-URL reporting.

### 2. `rel-to-abs` (npm/GitHub, archived June 2022)
- **API:** `convert(html, baseUrl)` — a single call, no options object.
- **Scope:** documented loosely as "any URL on links, images, scripts, etc."; the README does
  not state which attributes are covered, nor how `mailto:`/`tel:`/`data:`/`javascript:`,
  protocol-relative values, fragments or `srcset` are treated.
- **Status:** archived and read-only, so its behaviour is frozen.

### 3. `posthtml-urls` (npm, the reference implementation for "which attributes are URLs")
- **Coverage — the most complete list found**, and the one worth matching: `a[href,ping]`,
  `area[href,ping]`, `audio[src]`, `base[href]`, `blockquote[cite]`, `body[background]`,
  `button[formaction]`, `del[cite]`, `embed[src]`, `form[action]`, `frame[longdesc,src]`,
  `head[profile]`, `html[manifest]`, `iframe[longdesc,src]`, `img[longdesc,src,srcset]`,
  `input[formaction,src]`, `ins[cite]`, `link[href]`, `menuitem[icon]`,
  `meta[content]` (only when `http-equiv=refresh`), `object[codebase,data]`, `q[cite]`,
  `script[src]`, `source[src,srcset]`, `table|tbody|td|tfoot|th|thead|tr[background]`,
  `track[src]`, `video[poster,src]`, `applet[archive,code,codebase,object]`, and `*[itemtype]`.
- **Special-cased value shapes:** `srcset` (candidate list + descriptors), `ping`
  (multi-value), `meta content` (refresh directive) — each parsed on its own terms rather than
  treated as one opaque URL.
- **Options:** a transform callback + the ability to override the tag→attribute map; CSS
  `url()` is deliberately out of scope (a separate `posthtml-url-css` package handles it).
- **Surface:** a build-pipeline plugin — no page, no CLI for a paste-in user.

### 4. Ad-hoc JavaScript recipes (GeeksforGeeks article, DEV article, `cheerio` snippets)
- Two patterns dominate: hand-rolled `../`/`.` segment walking over split path arrays, and
  delegating to the DOM (`a.href` / the `URL` constructor) so the browser resolves it.
- The hand-rolled variants do not cover protocol-relative `//`, root-relative `/`, query-only
  or fragment-only values — a recurring source of subtle breakage the articles do not mention.
- The `cheerio` recipe re-serializes the document, so the output diff is not limited to URLs.

## Table stakes → what shipped

| Table stake (source) | Verdict | Where it landed |
| --- | --- | --- |
| Base-URL field, required, absolute (all) | in-model | `base`, validated: relative or non-hierarchical (`mailto:`) bases are rejected with an explanation |
| Rewrite `href` + `src` (all) | in-model | `attributes = "href-src"` tier, and the floor of every other tier |
| Resolve `../`, `./`, `/`, query-only (WillMaster, articles) | in-model | WHATWG `Url::join` via the `url` crate — same algorithm as the address bar; unit-tested |
| Skip values that already have a scheme (WillMaster, articles) | in-model | `kept:absolute` / `kept:scheme`; makes the tool idempotent |
| Skip bare fragments (WillMaster) | in-model | default `resolve_fragments = false`, with an opt-in for markup leaving its page |
| Full URL-attribute map (posthtml-urls) | in-model | `attributes = "common"` (default) and `"all"` tiers cover the whole list above |
| `srcset` candidate lists with descriptors (posthtml-urls) | in-model | per-candidate resolution, descriptors + spacing preserved |
| `ping` multi-value (posthtml-urls) | in-model | whitespace-separated list, each URL resolved independently |
| `meta http-equiv=refresh` content (posthtml-urls) | in-model | only the `url=` part is rewritten; delay and formatting untouched |
| Configurable attribute map (posthtml-urls) | in-model, simplified | three named tiers instead of a free-form map — a `<select>` beats a JSON blob on a page, and covers the same ground |
| CSS `url()` / `@import` (posthtml-url-css, a separate package there) | in-model | `style_urls` checkbox — off by default, so a run stays a pure attribute operation |
| In-place file edit + backup (WillMaster) | out-of-model | no server, no filesystem: the page is paste-in/copy-out, which is also why no backup is needed |
| Build-pipeline plugin API (posthtml-urls) | out-of-model | this is a tool surface, not a bundler plugin; the CLI covers scripted use |
| Fetch the page and convert it by URL (implied by WillMaster's file model) | out-of-model on the page | browser-local tools do not fetch cross-origin; `web-fetch` + this block composes it in chat/CLI |

## Gaps no competitor closes (built anyway)

- **`<base href>` awareness.** None of the four honours a document's own `<base>`, yet a
  browser resolves that document's relative URLs against it. Ignoring it produces URLs that
  point at the wrong place, silently. Shipped as `use_base_tag`, default on, with the `<base>`
  value itself made absolute and `base_tag_used` / `effective_base` reported.
- **Comment- and raw-text-awareness.** The regex/PHP approaches rewrite `href=` inside
  `<!-- … -->` and inside `<script>` strings. The scanner here treats those regions as text.
- **Template placeholders.** `{{ … }}`, `{% … %}`, `{# … #}`, `<% … %>`, `${ … }`, `[[ … ]]`
  are skipped, so the tool can be run on a template rather than only on rendered output.
- **Protocol-relative policy.** Left implicit everywhere else; here it is an explicit
  `protocol_relative` choice (resolve with the base's scheme, or keep).
- **Dry-run + report outputs.** No competitor shows what it decided. `output = "urls"` lists
  `line,tag,attribute,original,resolved,action` per URL; `output = "report"` gives the
  effective base plus rewritten/kept counts by reason and bytes before/after.
- **Byte-preserving contract.** The parser-based competitors re-serialize the document. Here
  only URL attribute values change, so the diff is reviewable.

## Considered, rejected

- **Absolute → relative (the inverse direction).** Not a pure transformation — it needs a
  per-URL policy (root-relative vs dot-segment vs leave-absolute for cross-origin), and
  guessing in bulk breaks internal links. Documented as an explicit non-goal in the FAQ.
- **A free-form tag→attribute map parameter** (posthtml-urls' `filter`). Rejected in favour of
  three tiers: a JSON map in a text box is a worse page control than a `<select>`, and the
  tiers already span the whole documented list.
- **Fetching the base page to discover its `<base>`/redirects.** Out of model (browser-local,
  no network) and unnecessary — a pasted document's own `<base>` is already honoured.

## Verification notes

Sources read: the WillMaster tool page, the `rel-to-abs` repository, `posthtml-urls`'
`lib/index.js` and `lib/defaultOptions.js` (npm's page returned HTTP 403, so the source was
read from the repository), and the GeeksforGeeks recipe article. The npm registry page for
`posthtml-urls` was unreachable and was replaced by its source, per the "replace unreachable
competitors" rule.
