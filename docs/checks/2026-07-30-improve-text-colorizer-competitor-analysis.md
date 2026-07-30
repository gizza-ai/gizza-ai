# text-colorizer — competitor analysis (2026-07-30)

**Tool:** Applies user-defined regex rules to highlight log or command output with
colors, exporting ANSI or HTML. Pure-Rust (`regex`), all surfaces (chat/CLI/page).

## Competitors scanned

1. **grc — generic colouriser** (github.com/garabik/grc). Config files list rules as
   `regexp=` + `colours=` (fore/back + attributes like bold/underline) blocks separated
   by blank lines, plus a `count=` keyword (`once`/`more`/`stop`/`block`/`unblock`).
   Ships bundled configs for many commands; colours are named (red, bold green, on_blue…).
2. **ccze / ccze-win** (scramblr/ccze-win). Perl/C log colouriser for syslog, apache,
   procmail; regex-based colour rules; **outputs colorized terminal text OR HTML** (`-h`).
   Rules colour whole tokens/fields; named colours.
3. **ChromaTerm** (pypi chromaterm). Reads stdin, colours per user-configurable YAML
   `rules:` array of `regex` + `color` (named or `f#rrggbb` fg / `b#rrggbb` bg) + optional
   `group`; works like a pipe (`cmd | ct`). Case-sensitive by default.
4. **txtstyle** (pypi txtstyle). `transforms` config of regex→style; colours matched
   substrings; named colours + attributes.
5. **onlinetexttools "Highlight Regexp Matches"** & **regex-highlighter** (JS libs). Web
   tools: paste text + a regex, pick a highlight colour, get colored inline HTML output;
   custom rules, HTML export.

## Table-stakes → decision

| Capability | Competitor(s) | Decision |
|---|---|---|
| User-defined `regex → color` rules, multiple | all | **in** — `rules`, one `color: regex` per line |
| Named ANSI colours (red, green, …, bright*) | grc, ccze, ChromaTerm | **in** — 8 basic + 8 bright + gray aliases |
| Hex/truecolor colours | ChromaTerm (`f#rrggbb`) | **in** — `#rgb`/`#rrggbb` → 24-bit ANSI / HTML hex |
| Attributes (bold/italic/underline/…) + background (`on <c>`) | grc, txtstyle | **in** — `bold red on white` style spec |
| ANSI terminal output | grc, ccze, ChromaTerm | **in** — `output=ansi` |
| HTML export | ccze `-h`, online tools | **in** — `output=html`, self-contained `<pre>` |
| Whole-line colouring (colour the matched line, not just token) | grc/ccze log mode | **in** — `whole_line` toggle |
| Case-insensitive matching | grc, ChromaTerm | **in** — `ignore_case` toggle |
| Rule priority / first-match-wins on overlap | grc `count=stop`, ChromaTerm order | **in** — earlier rule wins per character; documented |
| Light/dark HTML theme wrapper | ccze html, ansi-log-renderer | **in** — `theme` (dark/light) for HTML `<pre>` |
| Capture-group-only colouring | ChromaTerm `group` | **out (this cut)** — noted; per-rule group parsing adds a 3rd field; revisit |
| Bundled per-command presets (syslog/apache configs) | grc, ccze | **out-of-model** — shipping vendor config sets is out of scope; instead ship `[[example]]` preset chips (log-levels, IPs/URLs, diff) as starting points |
| Live stdin streaming / pipe | grc, ChromaTerm | **out-of-model** — batch text tool, not a stream; noted |

## Not a duplicate

- `ansi-log-renderer` interprets **existing** ANSI escape codes in the input → HTML; it
  does not apply user regex rules (it says "no regex"). text-colorizer is the inverse:
  plain text + user rules → ANSI/HTML.
- `syntax-highlighter` colours **source code** by language grammar (syntect), not by
  user-supplied regex rules.
- `regex-extract` returns a list of matches; it does not colour text or emit ANSI/HTML.

## Copy / branding

Paraphrase only; no competitor copy, config text, or trademarks copied. Preset chips are
original examples, not vendored grc/ccze configs.
