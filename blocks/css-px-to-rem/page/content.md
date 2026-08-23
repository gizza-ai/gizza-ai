## About this tool

**CSS px to rem Converter** rewrites the length units in a whole stylesheet, not just one number.
Paste CSS, SCSS, or LESS and every `px` length becomes `rem` — divided by the root font size you
choose — while comments, quoted strings, `url()` paths, selectors, and your exact indentation come
back untouched. Everything runs in your browser through WebAssembly; the CSS never leaves your
machine.

Rem lengths scale with the reader's browser font-size setting, so a layout built in rem respects
someone who has bumped their default text size up for readability — the main accessibility reason
teams migrate a px stylesheet in the first place.

You can also run it backwards (**rem → px**), which is handy when you inherit a rem-based theme and
need concrete pixel values for a design handoff.

### Options

- **Root font size** — what `1rem` stands for. Browsers default to `16`; pick `10` if your stylesheet
  uses the `html { font-size: 62.5% }` trick.
- **Decimal places** — 0–10, default 5. Trailing zeros are always trimmed, so `0.50000rem` prints as
  `0.5rem`.
- **Properties to convert** — `*` converts everything (default). Narrow it with a comma-separated
  list that accepts wildcards: `font*` (prefix), `*width` (suffix), `*margin*` (contains), an exact
  name like `line-height`, and a leading `!` to exclude — `*,!border*` converts everything except
  border properties.
- **Minimum px value** — leave small lengths alone. Setting `2` is the usual way to keep `1px`
  hairline borders crisp in px.
- **Convert inside @media conditions** — off by default, because breakpoints are commonly kept in px.
- **Skip rules whose selector contains** — comma-separated substrings; any rule (or block) whose
  selector matches is passed through in its original units.
- **Keep original as a fallback declaration** — emits the px declaration *and* the rem one as a pair.
- **Write zero as 0** — on by default, so `0px` becomes a bare `0` rather than `0rem`.

### Example

Input, with the defaults (root font size 16, all properties):

```css
.btn {
  font-size: 24px;
  padding: 8px 16px;
  margin: 0px;
  border: 1px solid #ccc;
}
```

Output:

```css
.btn {
  font-size: 1.5rem;
  padding: 0.5rem 1rem;
  margin: 0;
  border: 0.0625rem solid #ccc;
}
```

Set **Minimum px value** to `2` and that last line stays `border: 1px solid #ccc;` instead.

### Common conversions at a 16px root

| px | rem | | px | rem |
|---|---|---|---|---|
| 1px | 0.0625rem | | 20px | 1.25rem |
| 2px | 0.125rem | | 24px | 1.5rem |
| 4px | 0.25rem | | 28px | 1.75rem |
| 8px | 0.5rem | | 32px | 2rem |
| 12px | 0.75rem | | 40px | 2.5rem |
| 14px | 0.875rem | | 48px | 3rem |
| 16px | 1rem | | 64px | 4rem |

### Limits and edge cases

- **Declaration values only.** Selectors and at-rule preludes are never rewritten, except `@media`
  conditions when you opt in.
- **Case-sensitive unit match.** A capitalized unit is the per-value escape hatch: `16Px` and `16PX`
  are valid CSS and are left alone, so you can pin one value without touching the options.
- **Never touched:** text inside `/* comments */`, quoted strings, `url(...)` payloads, hex colors,
  and identifiers that merely contain the letters (`--gap-16px`, `translate3d`).
- **`em` is deliberately unsupported** — `em` resolves against the *parent* element's computed font
  size, which no stylesheet rewriter can know. Converting px→em against the root would produce
  numbers that look right and render wrong.
- It's a forgiving rewriter, not a validator: unbalanced braces are passed through as-is rather than
  rejected, so a malformed stylesheet comes back roughly as it went in.
- Very large stylesheets are limited only by your browser's memory — there is no server-side size cap
  because there is no server.

## FAQ

<details>
<summary>What root font size should I use — 16 or 10?</summary>

Use `16` unless your stylesheet deliberately sets a different root. Browsers default the `html`
element to 16px, so `1rem = 16px` out of the box. Some teams set `html { font-size: 62.5% }` (which
is 10px) purely so the arithmetic is mental — `24px` becomes `2.4rem`. If your project does that,
set the root font size here to `10` so the numbers match.

</details>

<details>
<summary>How do I keep 1px borders from becoming a fractional rem?</summary>

Set **Minimum px value** to `2`. Anything smaller than that stays in px, so `border: 1px solid`
survives while `padding: 24px` still converts. Sub-pixel rem borders can round to zero and disappear
at some zoom levels, which is why this option exists. Excluding them by property works too — put
`*,!border*` in the properties field.

</details>

<details>
<summary>Why did my media-query breakpoints stay in px?</summary>

Because converting them is off by default. Breakpoints in px are widely preferred since they compare
against the viewport rather than the root text size. Turn on **Convert inside @media conditions** if
you want `@media (min-width: 640px)` rewritten to `@media (min-width: 40rem)` — the declarations
*inside* a media block are converted either way.

</details>

<details>
<summary>Can I convert only the typography properties?</summary>

Yes — that's what the properties field is for. Enter something like `font*,line-height,letter-spacing`
and only those declarations are rewritten; everything else keeps its px values. The same field takes
exclusions with `!`, so `*,!letter-spacing` means "everything but that one".

</details>

<details>
<summary>What does the fallback option produce?</summary>

With **Keep original as a fallback declaration** on, each converted declaration is emitted twice —
the original px line first, then the rem line — so a browser that fails to apply the second one still
gets the first. The indentation of the original line is reused, so the pair lines up in the output.

</details>

<details>
<summary>Will it convert px to em instead?</summary>

No, and that's on purpose. `em` is relative to the parent element's computed font size, which depends
on where the rule lands in the DOM — a stylesheet rewriter has no way to know that. Any tool that
offers px→em is really just dividing by the root size, which is only correct when the parent happens
to be the root. Use rem when you want a predictable, document-wide scale.

</details>

<details>
<summary>Is my CSS uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly. The stylesheet you paste never
leaves your machine, so it's safe to use on unreleased or proprietary code.

</details>
