## About this tool

This tool rewrites pasted HTML so images and embeds get browser-native lazy-loading hints:
`loading="lazy"` for `<img>` and `<iframe>`, plus `decoding="async"` for images. It is meant for
bulk-cleaning snippets exported from CMSes, landing-page builders, documentation generators, or old
templates before you paste them back into a project.

The scanner is intentionally conservative. It only changes matching start tags and copies everything
else — comments, text, doctype, scripts, styles, attribute order, and quote style — through unchanged.
Existing `loading`, `decoding`, and `fetchpriority` attributes are never overwritten, so the transform
is idempotent.

## Worked example

Input:

```html
<img src="photo.jpg"><iframe src="https://example.com/embed"></iframe>
```

Default output:

```html
<img src="photo.jpg" loading="lazy" decoding="async"><iframe src="https://example.com/embed" loading="lazy"></iframe>
```

For Core Web Vitals work, set **First images to keep eager** to `1`, enable **Write loading="eager"**,
and enable **fetchpriority="high"** so the first image stays eligible for Largest Contentful Paint
while later images are deferred.

## Options

- **Elements to rewrite** targets images, iframes, or both.
- **Image decoding attribute** chooses `async`, `sync`, `auto`, or skips `decoding` entirely.
- **First images to keep eager** leaves the first N images in document order out of lazy loading.
- **Write `loading="eager"`** marks skipped first images explicitly instead of leaving them alone.
- **Add `fetchpriority="high"`** writes the priority hint on the first image if it lacks one.
- **Respect skip markers** leaves tags with `skip-lazy` / `no-lazy` classes or
  `data-skip-lazy` / `data-no-lazy` attributes untouched.
- **Output** returns either rewritten HTML or a change-count report.

## Limits

- The tool does not download images, compute dimensions, create placeholders, or infer which asset is
  actually above the fold. Use `skip_first` and `fetchpriority_first` when you know the hero image.
- It is a tolerant tag scanner, not a full browser parser. It is designed for normal HTML fragments
  and documents, not malformed markup recovery.
- Input is capped at 2 MB to keep browser runs responsive.
- `decoding` is only valid on images, so iframes never receive it.

## FAQ

<details>
<summary>Should every image get loading="lazy"?</summary>

No. The above-the-fold image, especially the Largest Contentful Paint image, should usually stay eager.
Set **First images to keep eager** to `1` (or more for a complex hero area). If you want an explicit
hint, also enable **Write loading="eager"** and **fetchpriority="high"** for the first image.

</details>

<details>
<summary>Will this overwrite attributes that are already there?</summary>

No. Existing `loading`, `decoding`, and `fetchpriority` values are preserved. That makes the output
safe to run again: tags already processed by this tool or tuned by hand do not keep changing.

</details>

<details>
<summary>How do I opt out a specific image or iframe?</summary>

Add a `skip-lazy` or `no-lazy` class, or a `data-skip-lazy` / `data-no-lazy` attribute. With
**Respect skip markers** enabled, those tags are copied through unchanged. Turn the option off only
when you intentionally want to rewrite every matching tag.

</details>

<details>
<summary>Does this optimize the image files themselves?</summary>

No. It only adds HTML attributes. Compressing, resizing, converting formats, and generating responsive
`srcset` variants are separate steps. This tool is useful after those steps, or when you only control
the markup and need a quick safe lazy-loading pass.

</details>
