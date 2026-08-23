## About this tool

**SVG to Data URI** turns inline SVG markup into a `data:image/svg+xml,...` URI you can paste into
CSS, HTML, JSX, or design-system tokens. The default output is the URL-encoded form because SVG is
text: keeping readable markup and escaping only the unsafe characters is usually shorter than
Base64. Pick Base64 when a downstream tool keeps rewriting or reformatting the URI.

The converter is SVG-aware rather than a generic byte encoder. It can minify the markup first,
rewrite double quotes to single quotes for shorter quoted `url("...")` snippets, and add the missing
root `xmlns="http://www.w3.org/2000/svg"` attribute that often makes SVG data URIs render blank in
CSS.

Everything runs locally in your browser. Your icon, logo, or pattern SVG is not uploaded.

### Example

Input:

```svg
<svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="#0af"/></svg>
```

With **Snippet** set to **CSS background-image**, the output starts like:

```css
background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E...");
```

### Options

- **Encoding** — URL-encoded (default) or Base64. URL encoding is normally smaller for SVG; Base64 is
  opaque and can be safer when a legacy pipeline mangles percent-encoded markup.
- **Snippet** — bare URI, CSS `background-image`, CSS `mask-image`, HTML `<img>`, JSX component, or a
  size comparison report.
- **Attribute quotes** — rewrite SVG attribute `"` to `'` (shorter) or percent-encode quotes as
  `%22` to keep the markup byte-identical.
- **Minify first** — strips XML declaration, DOCTYPE, comments, and redundant whitespace before
  encoding. Turn it off when `<text>` content intentionally relies on repeated spaces.
- **Add xmlns when missing** — injects the default SVG namespace into the root element, which is
  usually required for data-URI SVGs to render inside CSS `url()`.

### Limits and edge cases

- The input must contain a root `<svg>` element. If you already have a `data:` URI and want markup
  back, decode it with the data URI decoder instead.
- SVG input is capped at 1,000,000 bytes. Large illustrations are better served as external `.svg`
  files so browsers can cache them.
- Minification is intentionally simple: it removes invisible wrapper/comment bytes and collapses
  whitespace. It does not optimize paths, merge shapes, or rewrite colors; run an SVG optimizer first
  if you need structural compression.
- The URL-encoded form escapes characters that break quoted CSS/HTML contexts (`<`, `>`, `#`, `%`,
  `&`, braces, control characters, and optionally `"`). Spaces and many punctuation characters stay
  readable because browsers accept them inside a quoted data URI.
- Inline data URIs duplicate bytes everywhere they are used. For repeated large assets, a normal SVG
  file can be faster because it is cached once.

## FAQ

<details>
<summary>Should I choose URL-encoded SVG or Base64?</summary>

Choose URL-encoded unless you have a specific compatibility problem. SVG is text, so escaping only
the unsafe characters usually produces a shorter and more readable URI than Base64, which adds about
one third to the payload size. Use the **Size comparison** output when you want to confirm the exact
winner for one SVG.

</details>

<details>
<summary>Why does the tool add an xmlns attribute?</summary>

Browsers often require the root SVG namespace when an SVG is loaded from a `data:` URI, especially in
CSS `background-image` and `mask-image`. An ordinary inline `<svg>` can appear to work without it,
then render blank when encoded. Leaving **Add xmlns when missing** on prevents that failure.

</details>

<details>
<summary>Can I use the result in CSS masks?</summary>

Yes. Set **Snippet** to **CSS mask-image** and the tool emits both `-webkit-mask-image` and the
standard `mask-image` declaration. That covers Safari/WebKit as well as browsers that support the
unprefixed property.

</details>

<details>
<summary>Will minify change my drawing?</summary>

For icon-style SVGs, no: it removes declarations, comments, and redundant whitespace that do not
change rendering. Turn minification off if your SVG includes `<text>` nodes where repeated spaces are
meaningful, or if you need the encoded markup to match the original bytes exactly.

</details>

<details>
<summary>Is this the same as a generic data URI encoder?</summary>

No. A generic encoder can wrap any bytes in a `data:` URI. This tool is specialized for SVG: it uses
the SVG MIME type, adds the namespace when needed, can minify markup first, offers CSS/HTML/JSX
snippet shapes, and compares URL encoding against Base64 for the same SVG.

</details>
