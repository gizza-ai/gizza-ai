# font-subset — competitor analysis (2026-07-26)

Tool function: accept a font file plus the text it needs to render, keep only the glyphs required for that text, and return a smaller downloadable font suitable for embedding.

## Competitors surveyed

1. **Font Squirrel Webfont Generator** — accepts font uploads, lets users choose subsetting modes, custom character ranges, and webfont formats. Strong UI around licensing reminders and CSS output.
2. **Everything Fonts Font Subsetter** — upload a font, provide characters/text to keep, and download a subset font. Focuses on size reduction and common webfont workflows.
3. **Glyphhanger / subfont CLI workflows** — scan text/pages, subset webfonts to used glyphs, and emit WOFF2/CSS for deployment pipelines.
4. **fonttools pyftsubset** — command-line baseline with text/unicode selection, layout closure, flavor output, and many expert flags.

## Table-stakes params → our descriptor

| Capability | Competitors | In model? | Our param / behavior |
|------------|-------------|-----------|----------------------|
| Upload/provide a source font | all | yes | `url` or `ref` via `Input::File` |
| Text/characters to keep | all | yes | required `text` |
| Compressed webfont output | all web tools | yes | `format=woff2` default |
| Raw OpenType output | CLI tools | yes | `format=opentype` |
| Drop variable-font axes | CLI tools | yes | `drop_variations` |
| Missing glyph reporting | CLI tools | yes | result summary lists missing character count/preview |
| Browser drag-and-drop page | web tools | partial/out | no generic page for binary font-in/font-out in this repo pattern; chat+CLI return a downloadable envelope |
| CSS generation / @font-face snippets | web tools | out | site chrome and naming policy; not part of the block model |
| URL/page crawling to discover text | Glyphhanger/subfont | out | network crawler/site integration, not a pure font subset transform |
| Advanced OpenType layout closure flags | pyftsubset | out | crate handles normal character/glyph closure; expert table surgery is intentionally not exposed |

Defaults: WOFF2 output for web-size savings; variable tables preserved unless explicitly dropped.

## UX/control decisions

- `text` is a required string because every competitor centers the workflow on the characters that must remain.
- `format` is an enum (`woff2`, `opentype`) rather than free text so CLI/chat validation is deterministic.
- `drop_variations` is a non-default checkbox/boolean for users who understand the tradeoff.
- No standalone page/spec is shipped because this repository's established no-page file-output pattern is used for binary file-in/binary file-out tools such as `woff2-convert`.

No competitor copy, branding, or trademarks were used; capabilities above are paraphrased and mapped to the gizza model.
