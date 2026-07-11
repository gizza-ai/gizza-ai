## About this tool

Metadata Privacy Linter checks an image for embedded metadata that can leak private
information when you share a photo or screenshot. Paste base64 image bytes (or a
`data:image/...;base64,...` URL) and it scans the container for EXIF/TIFF, XMP,
and IPTC/IIM fields.

It flags common privacy risks:

- **GPS location** — coordinates, GPS timestamps, and GPS processing hints.
- **Device identifiers** — camera/lens serials, camera owner names, model names.
- **Personal identity** — creator, by-line, copyright, credit, and owner fields.
- **Timestamps** — capture, digitized, created, and modified dates.
- **Software and descriptions** — editing apps, host computer, captions, keywords.

Values are hidden by default so you can share the report without re-leaking the
metadata you are trying to remove. Turn on **Reveal concrete metadata values** to
show field values, decoded GPS coordinates, and an OpenStreetMap link.

### Worked example

A PNG containing XMP creator and serial metadata:

```text
image_base64 = iVBORw0KGgpwcmVmaXg8eDp4bXBtZXRh...
min_risk = all
reveal_values = false
output = report
```

The report lists high-risk creator/device fields without printing the actual name
or serial number. Switch `output` to `json` when you need structured data for a
pipeline.

## FAQ

<details>
<summary>Is the image uploaded?</summary>

No. The scan runs in WebAssembly in your browser. The pasted base64/data URL is
processed locally and never uploaded by this page.

</details>

<details>
<summary>Why does the tool ask for base64 instead of a file picker?</summary>

This gizza page model is text-input based, so base64/data URLs are the portable
way to pass exact image bytes through the CLI, chat tool, and browser page. Use a
local encoder or a previous tool output to provide the bytes.

</details>

<details>
<summary>What metadata formats are scanned?</summary>

The linter detects common image containers (JPEG, PNG, TIFF, WebP, HEIF, GIF) and
scans EXIF/TIFF, XMP packets, and JPEG IPTC/IIM resources when those metadata
blocks are present.

</details>

<details>
<summary>Does it remove the metadata?</summary>

No. It is a linter: it tells you what would leak and how risky each field is. Use
an image metadata stripper or re-export workflow to remove the fields after you
identify them.

</details>

<details>
<summary>Why are values hidden by default?</summary>

A privacy report can itself become sensitive if it prints GPS coordinates, serial
numbers, or names. Hidden values let you share the report safely; reveal values
only when you need exact coordinates or field contents.

</details>
