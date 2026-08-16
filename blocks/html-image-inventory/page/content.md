## About this tool

Paste the HTML of a page, a template fragment, or an email body and this tool walks the markup with
a real HTML parser and builds **one table row for every image source it declares** — every `<img>`,
and every `<picture><source>` candidate behind it — together with the attributes that decide how
that image renders:

- **Source** — `src`, plus `srcset` and `sizes` for responsive images, and `media` + `type` for each
  `<picture>` candidate so art-direction breakpoints and AVIF/WebP fallbacks are visible.
- **Alt text** — reported in three states, not two: **present**, **empty** (`alt=""`, the correct
  markup for a decorative image), or **missing** (no `alt` attribute at all, which is the real
  accessibility failure).
- **Dimensions** — the `width` and `height` content attributes, exactly as written.
- **Loading hints** — `loading`, `decoding`, and `fetchpriority`.
- **Locators** — `class`, `id`, and `title`, so you can find the element again in your source.

Two defects are flagged, both of them named audit rules rather than opinions:

- **`missing-alt`** — the `<img>` has no `alt` attribute. A screen reader falls back to announcing
  the file name. An explicit `alt=""` is *not* flagged by default; it is how you correctly mark an
  image as decorative.
- **`missing-width` / `missing-height`** — the `<img>` has no usable dimension attribute, so the
  browser reserves no space for it and the page jumps as it loads. That is a Cumulative Layout Shift
  contributor, and CLS is a Core Web Vital.

A third, `no-source`, catches an `<img>` with neither `src` nor `srcset` — usually a broken template
variable.

### Worked example

Input HTML:

```html
<article>
  <img src="/hero.jpg" alt="Team on a rooftop" width="1200" height="800" decoding="async" fetchpriority="high">
  <img src="/promo.png" width="600" height="400" loading="lazy">
  <img src="/divider.svg" alt="" width="8" height="8">
  <img src="/chart.png" alt="Revenue by quarter" loading="lazy" decoding="async">
</article>
```

Output with **Markdown** selected:

```markdown
## Image inventory

4 images, 1 missing alt, 1 missing dimensions, 1 decorative (alt=""), 2 lazy-loaded

| # | Element | Source | Alt | Size | Loading | Decoding | Issues |
|---|---------|--------|-----|------|---------|----------|--------|
| 1 | img | `/hero.jpg` | Team on a rooftop | 1200×800 | — | async | — |
| 2 | img | `/promo.png` | — | 600×400 | lazy | — | missing-alt |
| 3 | img | `/divider.svg` | *(decorative)* | 8×8 | — | — | — |
| 4 | img | `/chart.png` | Revenue by quarter | — | lazy | async | missing-width, missing-height |
```

Row 3 is clean even though its alt is empty — that divider is decorative and correctly marked.
Row 4 is the layout-shift problem: it has good alt text but no dimensions at all. Turn on **Only
flagged images** to get just rows 2 and 4, or switch to **CSV** for a spreadsheet and **JSON** for a
script.

### Good uses

- **Fix an accessibility report** — get the exact list of images with no `alt`, with their `id` and
  `class` so you can find each one in your templates.
- **Chase a layout shift** — every image with no explicit `width`/`height` is the first place to
  look when Cumulative Layout Shift is high.
- **Review a responsive image set** — see every `<picture>` candidate's `srcset`, `media` query, and
  `type` side by side, and confirm the AVIF/WebP fallback chain ends at a real `<img>`.
- **Check lazy-loading strategy** — spot the above-the-fold hero that was accidentally given
  `loading="lazy"` (which delays your Largest Contentful Paint) or the below-the-fold gallery that
  was not.
- **Hand a writer the alt-text worklist** — export CSV, delete the rows that already pass, and send
  the rest.
- **Document a template** — paste the Markdown table straight into a PR, a ticket, or a README.

### Limits and edge cases

- Reports at most **2000 rows** per run. Past that you get an error naming the cap — the table is
  never silently truncated. Paste one page or section at a time, or turn off **Include `<picture>`
  sources**.
- Everything runs locally in your browser via WebAssembly. Nothing is uploaded, and the tool has no
  network access — it cannot fetch a URL for you, so paste the markup (View Source, `curl`, or your
  browser's DevTools → Elements → Copy → Copy outerHTML).
- Attributes are reported **as written**. The image files themselves are never downloaded, so real
  pixel dimensions, real file weight, real format, and broken/404 sources are all out of scope. A
  `width="1200"` on a 300-pixel-wide file is reported as `1200`.
- **CSS `background-image`** and images injected by JavaScript are not `<img>` elements and do not
  appear. Copy the rendered DOM from DevTools rather than View Source to capture JS-added images;
  CSS backgrounds are invisible to any markup parser and also carry no alt text by design.
- `<source>` elements inside `<video>` and `<audio>` are ignored — they are media, not images. Only
  `<source>` inside a `<picture>` is inventoried.
- Inline `<svg>` graphics and `<canvas>` are not image *sources* and are out of scope; their
  accessible names come from `<title>`/`aria-label` instead.

## FAQ

<details>
<summary>Why is an image with alt="" not flagged?</summary>

Because an empty `alt` is correct markup, not a mistake. `alt=""` tells a screen reader "this image
carries no information, skip it" — the right thing for spacers, dividers, decorative flourishes, and
an icon that sits next to a text label that already says the same thing. Removing the attribute
entirely is what causes trouble: with no `alt` at all, many screen readers fall back to reading out
the file name, so a user hears "I M G underscore 4 0 3 2 dot jpeg".

The tool therefore reports three states — `present`, `empty`, and `missing` — and only flags
`missing`. If you are auditing whether each decorative marking was deliberate, turn on **Flag
decorative alt="" too** and every empty alt is listed as well.

</details>

<details>
<summary>Why is width="50%" flagged as missing?</summary>

Because the `width` and `height` **content attributes** in HTML must be valid non-negative integers
— unitless numbers of pixels. `50%`, `auto`, and `10.5` are not, so the browser cannot use them to
work out the image's aspect ratio and reserve space for it before the file arrives. The page still
shifts as the image loads, which is exactly the problem the check exists to catch.

The value is reported verbatim in the table so you can see what is actually there, and flagged so it
does not quietly pass. The fix is to put the intrinsic pixel size in the attributes and do the
proportional sizing in CSS (`width: 50%; height: auto`) — the two work together, and modern browsers
derive the aspect ratio from the attributes.

</details>

<details>
<summary>How are &lt;picture&gt; and &lt;source&gt; handled?</summary>

Each `<picture>` is numbered, and every `<source>` inside it becomes its own row labelled
`source (picture N)`, followed by the `<img>` fallback labelled `img (picture N)` — the same order
as the markup, which is also the order the browser evaluates the candidates in.

A `<source>` row shows its `srcset`, its `media` query, and its `type`, all listed under
**Responsive sources** below the table where there is room for the full value. It shows `n/a` in the
Alt column, because a `<source>` has no `alt` of its own — the `<img>` at the end of the `<picture>`
supplies the alt text for whichever candidate the browser picks, so that is the only row where alt
is audited.

Turn off **Include `<picture>` sources** to collapse the table down to just the `<img>` rows; the
count of sources found still appears in the summary.

</details>

<details>
<summary>Can it read the images from a URL instead of pasted HTML?</summary>

No. This tool is fully offline — it has no network access at all, which is why your markup never
leaves your machine. That also means it never downloads the image files, so it reports what the
attributes *say* rather than what the files *are*: real pixel dimensions, real file size, real
format, and broken links are all outside what a markup parser can honestly tell you.

Fetch the page yourself (View Source, `curl`, or DevTools → Elements → Copy outerHTML) and paste the
result here. Pasting is often the only option anyway for a staging site behind a login, a CMS
template fragment, or an HTML email — none of which a URL-based checker can reach.

</details>

<details>
<summary>Does it handle messy or invalid HTML?</summary>

Yes. Parsing uses the same HTML5 parsing algorithm browsers use, so unquoted attribute values
(`<img src=/a.png alt=Hello>`), unclosed `<p>` and `<li>` tags, uppercase tag and attribute names,
and sloppy nesting all parse the way a browser would read them. That is the main reason to use this
rather than a regular expression over `<img[^>]*>`, which breaks on all of the above — and cannot
see the `<picture>` → `<source>` → `<img>` structure at all, since that relationship lives in the
tree rather than in any single tag.

</details>

<details>
<summary>What do the loading, decoding, and fetchpriority columns tell me?</summary>

They are the browser hints that decide *when* each image is fetched and painted, and they are
reported verbatim so you can check them against your intent:

`loading="lazy"` defers the fetch until the image is near the viewport — right for everything below
the fold, and wrong for your hero, because lazy-loading the largest above-the-fold image directly
delays Largest Contentful Paint. `decoding="async"` lets the browser decode the image off the main
thread so it does not block rendering. `fetchpriority="high"` pushes one image to the front of the
queue, and is normally used on exactly one image per page — the LCP one.

A common finding: a hero row showing `lazy` in the Loading column, or every image on the page
showing `fetchpriority=high`, which means none of them is actually prioritised.

</details>

<details>
<summary>How do I get just the problems, without the images that already pass?</summary>

Turn on **Only flagged images**. The table then contains only rows with at least one issue token,
renumbered from 1, and a line underneath tells you how many clean rows were hidden so the totals
still add up. The summary above the table always counts every image found, filtered or not.

If nothing is flagged you get "No issues found — every image has alt text and explicit dimensions"
rather than an error, so the filter is safe to leave switched on in a script or a deep link.

</details>
