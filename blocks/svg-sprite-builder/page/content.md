## About this tool

The SVG sprite sheet builder turns a pile of individual `<svg>` icons into a
single SVG **symbol sprite** — one `<svg>` containing a `<symbol>` per icon. You
include that sprite once in your page, then draw any icon anywhere with a tiny
`<use>` reference:

```html
<!-- once, near the top of <body> -->
<svg aria-hidden="true" style="position:absolute;width:0;height:0">
  <symbol id="icon-1" viewBox="0 0 24 24">…</symbol>
  <symbol id="icon-2" viewBox="0 0 16 16">…</symbol>
</svg>

<!-- anywhere you want the icon -->
<svg class="icon"><use href="#icon-1" /></svg>
```

This keeps your markup small, lets you style every instance with CSS, and avoids
extra network requests for each icon.

### How to use it

1. Paste one or more complete `<svg>…</svg>` documents into the box, one after
   another (the order is preserved).
2. Pick how ids are named: **auto** (`prefix-1`, `prefix-2`, …), the source
   SVG's **id** attribute, or its **title** text. Missing ids fall back to auto,
   and duplicate ids are disambiguated with a numeric suffix.
3. Set a **prefix** for the auto ids (default `icon`).
4. Leave **Hidden wrapper** on so the sprite renders nothing where you drop it.

Each symbol keeps its `viewBox`; if a source SVG only has `width`/`height`, a
`viewBox` of `0 0 W H` is derived for you. The output ends with a comment listing
the ready-to-paste `<use>` snippet for every symbol.

Everything runs locally in your browser — your SVGs are never uploaded.
