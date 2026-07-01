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

## FAQ

<details>
<summary>How are the symbol ids chosen, and what about duplicates?</summary>

Three naming modes: **auto** numbers them `prefix-1`, `prefix-2`, … (prefix
default `icon`); **id** reuses each source SVG's `id` attribute; **title** slugs
the `<title>` text. An icon missing the chosen attribute falls back to auto
naming, and any duplicate id gets a numeric suffix — so `<use>` references are
always unique.

</details>

<details>
<summary>My icons only have width/height, no viewBox — will they still scale?</summary>

Yes. When a source SVG lacks a `viewBox` but has pixel `width`/`height`
attributes, a `viewBox="0 0 W H"` is derived automatically (a trailing `px` is
fine). Percentage or em/rem sizes can't be converted to a viewBox, so such icons
are left without one — add a real viewBox to the source if scaling matters.

</details>

<details>
<summary>How do I actually use the generated sprite?</summary>

Paste the sprite once near the top of `<body>` — with **Hidden wrapper** on it
carries `aria-hidden` and zero-size styling, so it renders nothing by itself.
Then draw any icon with `<svg class="icon"><use href="#icon-1" /></svg>`. The
output even ends with a comment listing the ready-made `<use>` snippet for every
symbol.

</details>

<details>
<summary>What happens if one of my pasted SVGs is malformed?</summary>

You get a specific error — "no &lt;svg&gt; element found", an opening tag that's
never closed, or a missing `</svg>` — instead of a silently truncated sprite.
Fix the offending icon and rebuild; the order of the pasted documents is always
preserved in the output.

</details>
