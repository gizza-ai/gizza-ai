## About this tool

The CSS Autoprefixer adds the vendor prefixes that current browsers still need to your
CSS. Paste a stylesheet, and for every declaration that benefits from a prefix the tool
inserts the prefixed clones immediately **before** the original — so the standard form
comes last and wins on any fully-supporting browser. Everything runs locally in your
browser; your CSS is never uploaded.

## What it prefixes

- **Property prefixes** — `user-select`, `appearance`, `backdrop-filter`, `clip-path`,
  `mask` (and `mask-*`), `hyphens`, `text-size-adjust`, `box-decoration-break`,
  `writing-mode`, `text-orientation`, `font-feature-settings`, `touch-action`,
  `print-color-adjust`, and more.
- **Property renames** — `tab-size` becomes `-moz-tab-size` / `-o-tab-size`;
  `background-clip: text` adds `-webkit-background-clip: text`.
- **Value prefixes** — `display: flex` expands to `-webkit-box`, `-webkit-flex` and
  `-ms-flexbox`; `display: inline-flex` and `display: grid` similarly; `position: sticky`
  adds `-webkit-sticky`; intrinsic sizes (`width: max-content` etc.) add the
  `-webkit-`/`-moz-` forms.

## Safe and idempotent

Declarations inside `/* comments */` and quoted strings, plus CSS custom properties
(`--my-var`), are left exactly as written. By default the tool is **idempotent**: if a
prefixed declaration is already present in a rule, it is not duplicated, so you can run it
again on already-prefixed CSS without bloating it. Uncheck the idempotent option to emit
every prefix unconditionally.

## Notes

The prefix set is curated for **current** browser targets, not an exhaustive list of every
historical prefix ever shipped. It is intended to cover the prefixes you still need today,
keeping the output lean.
