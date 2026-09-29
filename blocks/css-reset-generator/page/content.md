## About this tool

Build a CSS reset stylesheet from small, named sections instead of copying a fixed snippet you then have to edit by hand. Start with a preset such as `modern`, `minimal`, `normalize`, `classic`, `preflight`, or `none`, then add or remove specific sections like `focus-visible`, `smooth-scroll`, `lists`, `button-reset`, or `tables`.

The generated CSS is deterministic and plain text. You can keep explanatory section comments for review, turn on minified output for a compact single-line sheet, wrap the reset in a cascade `@layer`, or emit zero-specificity `:where()` selectors so later project CSS overrides the reset naturally.

### Worked example

A layered reset that keeps the modern preset, adds focus and smooth-scroll opinions, drops font smoothing, and uses dynamic viewport height:

```bash
gizza tool css-reset-generator 'preset=modern' 'include=focus-visible smooth-scroll' 'exclude=font-smoothing' 'selector_style=where' 'layer=base.reset' 'body_min_height=100dvh'
```

That produces an `@layer base.reset { ... }` stylesheet containing reset sections such as border-box sizing, media defaults, form inheritance, balanced headings, reduced-motion protection, `:focus-visible`, and smooth scrolling guarded by the user's motion preference.

### Limits and edge cases

- The section vocabulary is fixed so outputs stay predictable. Unknown section ids are rejected with the full list of accepted ids.
- `line_height` must be between `1` and `3`; `indent` must be a whole number from `0` to `8`.
- `minify=true` disables comments and indentation because the output is intentionally a single line.
- Presets are style-alike baselines authored for this tool. They are not byte-for-byte copies of third-party reset or normalize projects.
- `:where()` selectors keep pseudo-elements such as `*::before` outside the wrapper because pseudo-elements are not valid inside `:where()`.

## FAQ

<details>
<summary>Should I use the modern preset or minimal preset?</summary>

Use `modern` when you want a broad opinionated baseline for an app or design system. Use `minimal` when you only want the safest foundation: border-box sizing, margin removal, responsive media, and form controls inheriting typography.

</details>

<details>
<summary>What is the difference between include and exclude?</summary>

`include` turns on extra sections after the preset is chosen. `exclude` removes sections after that, so exclude wins if the same section appears in both fields. Sections always render in the tool's canonical order rather than the order you typed.

</details>

<details>
<summary>Why would I wrap the reset in an @layer?</summary>

A cascade layer lets your project place the reset below components and utilities on purpose. For example, `--layer base.reset` emits `@layer base.reset { ... }`, making it easier for later layers to override reset rules without adding selector weight.

</details>

<details>
<summary>Does the tool copy Normalize.css, Preflight, or a named reset?</summary>

No. The presets follow common reset patterns, but the emitted rules are authored here and assembled from named sections. If you need an exact upstream stylesheet, use that project's official distribution instead.

</details>
