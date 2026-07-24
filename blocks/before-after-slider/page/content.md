## About this tool

A **before/after image slider** overlays two pictures and lets a viewer drag a divider to
wipe between them — the classic way to show a photo edit, a renovation, a map change, or a
retouch. This generator turns two image sources into a **single self-contained HTML file**:
inline CSS and JavaScript, no jQuery, no CDN, no build step. Save it as `.html` and open it,
or paste the embed snippet straight into a web page or CMS.

Everything runs locally in your browser — the images are referenced by the URL or `data:`
URI you provide, nothing is uploaded to a server, and no account is needed.

### What you get

- **Drag, hover, touch, or keyboard.** Drag the handle, or turn on *move-on-hover*. On a
  phone it responds to touch. The divider is a focusable ARIA slider: **←/→** (or **↑/↓**)
  nudge it by 2%, **Shift** by 10%, and **Home/End** jump to the edges.
- **Horizontal or vertical wipe.** A vertical divider that moves left↔right, or a horizontal
  one that moves up↕down.
- **Custom start position.** Set where the divider begins (0–100%); default is the middle.
- **Per-side labels.** "Before" / "After" badges you can rename or hide (leave a label blank
  to remove it).
- **Your divider color**, a responsive fluid width or a fixed max width, and **multiple
  sliders on one page** — the inline script initializes every widget it finds.

### Worked example

**Input**

- Before image: `https://picsum.photos/id/1015/900/600`
- After image: `https://picsum.photos/id/1016/900/600`
- Orientation: `horizontal`, Start position: `50`, Output: `document`

**Output** — a complete HTML page (trimmed) whose body holds the widget:

```html
<div class="bas-container" role="slider" tabindex="0" ... style="--pos:50%">
  <img class="bas-after"  src="https://picsum.photos/id/1016/900/600" alt="After">
  <img class="bas-before" src="https://picsum.photos/id/1015/900/600" alt="Before">
  <span class="bas-tag bas-tag-before">Before</span>
  <span class="bas-tag bas-tag-after">After</span>
  <div class="bas-divider"><div class="bas-handle"></div></div>
</div>
```

Copy the whole result, save it as `slider.html`, and open it — or choose **Embed** output to
get just the `<style>` + `<div>` + `<script>` to drop into an existing page.

### Good to know / limits

- The two images should share the **same dimensions** (or at least aspect ratio) for a clean
  wipe — the "before" layer is fitted over the "after" layer with `object-fit: cover`.
- Sources must be an `http(s)://` URL, a relative path, or a `data:image/...` URI. For a
  truly portable, offline file, use base64 `data:` URIs so the images travel inside the HTML.
- `javascript:` / `vbscript:` / `file:` sources and non-image `data:` URIs are rejected, and
  labels + image URLs are HTML-escaped, so the generated file can't smuggle script.

## FAQ

<details>
<summary>How do I use the generated file?</summary>

Pick **Standalone HTML page** and the result is a full document — copy it, save it as
`slider.html`, and double-click to open it in any browser. Pick **Embed snippet** and you get
just the `<style>`, `<div>`, and `<script>`; paste that into your page's HTML (a blog post,
CMS block, or landing page) where you want the slider to appear.

</details>

<details>
<summary>Where do the images come from — do I upload them?</summary>

Nothing is uploaded. You pass two image **sources**: an `http(s)://` URL (the HTML links to
it) or a `data:image/...;base64,...` URI (the image bytes are embedded, so the file is fully
self-contained and works offline). Both `before` and `after` accept either form.

</details>

<details>
<summary>Can I put several sliders on the same page?</summary>

Yes. Each embed snippet uses the same `bas-container` class and the inline script
initializes **every** slider it finds on the page, so you can paste multiple snippets. The
script guards against double-initialization, so duplicate `<script>` copies are harmless.

</details>

<details>
<summary>My two images look misaligned or stretched — why?</summary>

The wipe overlays one image on the other, so they need the **same dimensions** (or aspect
ratio) to line up. The "after" image defines the box and the "before" image is fitted over it
with `object-fit: cover`; if their aspect ratios differ, the top layer is cropped to fit.
Re-export both images at the same size for a pixel-perfect comparison.

</details>

<details>
<summary>Is it keyboard accessible?</summary>

Yes. The divider is a focusable element with `role="slider"` and live `aria-valuenow`. Tab to
it, then use **←/→** or **↑/↓** to move it by 2%, hold **Shift** for 10% steps, and press
**Home** / **End** to jump to either edge.

</details>

<details>
<summary>Does it work on touchscreens?</summary>

Yes — it uses pointer events with `touch-action: none`, so a finger drag moves the divider on
phones and tablets. Move-on-hover applies to mice; touch always drags.

</details>
