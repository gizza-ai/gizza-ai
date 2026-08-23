## About this tool

Print workflows store images in **CMYK** — four ink channels (cyan, magenta, yellow and black)
instead of the three light channels screens use. Photoshop's "Save as JPEG" from a CMYK document
writes exactly that: a four-component JPEG, usually in the **Adobe YCCK** flavour marked by an
`Adobe` APP14 segment. Screens, browsers, CMSes and most image libraries expect **RGB**, so those
files get refused on upload, render with muddy or inverted colours, or come back with a stray
alpha channel.

This tool decodes the four ink channels and writes a true three-channel RGB image — as lossless
**PNG**, as **JPEG**, or as **WebP**. Everything runs locally in your browser through a
WebAssembly build of ffmpeg; nothing is uploaded to a server.

The extra care that makes this different from a plain format converter: a naive transcode of a
CMYK JPEG maps the **black (K) channel into an alpha slot** — ffmpeg reads Adobe YCCK as
`yuva444p` and plain Adobe CMYK as `gbrap` — so you end up with an RGBA file carrying a pointless
fully-opaque alpha channel. This tool pins the output pixel format (`rgb24` for PNG, `yuvj420p` or
`yuvj444p` for JPEG), so what you download is genuinely three-channel RGB.

### Worked example

Take a 64×64 CMYK JPEG of four quadrants — pure cyan, pure magenta, pure yellow, and pure black
ink — exported as Adobe YCCK.

**Input:** `quadrants.jpg` — 64×64, 4 components, Adobe APP14 transform 2 (YCCK)

**Settings:** format `png`, chroma `4:2:0` (ignored for PNG), quality `90` (ignored for PNG)

**Output:** `quadrants-rgb.png` — 64×64, pixel format `rgb24`, no alpha channel, with the
quadrants now reading as ordinary RGB:

| Quadrant | CMYK ink | RGB result |
|---|---|---|
| top-left | C 100% | `rgb(0, 255, 255)` cyan |
| top-right | M 100% | `rgb(255, 0, 255)` magenta |
| bottom-left | Y 100% | `rgb(255, 255, 0)` yellow |
| bottom-right | K 100% | `rgb(0, 0, 0)` black |

Run the same file with format `jpeg` and chroma `4:4:4` and you get a JPEG whose colour is stored
at full resolution — visibly cleaner on the hard edges between those quadrants than the 4:2:0
default.

### Settings

- **Output format** — `png` (default) is lossless and the right choice for logos, type and flat
  colour, which is most CMYK artwork. `jpeg` suits photographs. `webp` gives the smallest file.
- **Quality (1–100)** — applies to JPEG and WebP only; PNG is lossless and ignores it. The default
  is 90. 70–85 is a good size/quality trade; 100 is near-lossless.
- **Chroma subsampling** — applies to JPEG output only. `4:2:0` (default) stores colour at half
  resolution for the smallest file. `4:4:4` keeps colour at full resolution, which matters for the
  coloured text, logos and flat fills typical of print files. PNG is always full RGB and WebP is
  always 4:2:0.

### Limits and edge cases

- **Maximum size:** 8 MiB in and 8 MiB out. Larger files are rejected with an explicit error
  rather than being silently truncated.
- **Accepted inputs:** PNG, JPEG and WebP. TIFF, GIF, PDF, EPS, PSD and camera RAW are **not**
  accepted — a CMYK TIFF or a PDF has to be exported to JPEG first.
- **Already-RGB files are accepted**, not rejected: they are simply re-encoded in the format you
  pick. On the chat and CLI surfaces the result line tells you which happened, so a re-encode is
  never reported as a conversion.
- **The conversion is arithmetic, not an ICC-profiled press proof.** Output is untagged sRGB,
  which is what browsers assume. If your file carries a specific press profile (SWOP, FOGRA) and
  you need a colour-managed match, use a colour-management application instead — expect small
  shifts in the most saturated inks either way.
- **CMYK has a smaller gamut than RGB in some areas and a larger one in others**, so rich blacks
  and saturated inks can look flatter after conversion. That is inherent to the colour spaces, not
  to this tool.
- **Animated inputs** (animated WebP, GIF-derived files) convert their first frame — the output is
  a single still image.
- **One file per run.** There is no batch mode.

## FAQ

<details>
<summary>Why won't my CMYK JPEG open, or why do the colours look wrong?</summary>

Most software assumes a JPEG has three components (RGB or YCbCr). A CMYK JPEG has four, and the
Adobe variants store them inverted with an `Adobe` APP14 marker that a decoder has to know about.
Decoders that don't either refuse the file, render it inverted or muddy, or drop the black channel.
Converting to RGB once, up front, removes the guesswork for every program downstream.

</details>

<details>
<summary>Will converting CMYK to RGB change my colours?</summary>

Slightly, and unavoidably. CMYK and RGB describe colour in different ways and neither fully
contains the other, so the most saturated inks — deep reds, rich blacks, strong cyans — shift when
mapped to screen colours. This tool does the standard arithmetic conversion to untagged sRGB. It
does not read your file's press profile, so treat the result as a web-ready image, not a
colour-proofed one.

</details>

<details>
<summary>Which output format should I pick?</summary>

`png` unless you have a reason not to. It is lossless, so nothing degrades, and CMYK sources are
usually artwork with type and flat colour where JPEG artefacts show up quickly. Pick `jpeg` when
the image is a photograph and file size matters, and `webp` when you want the smallest file for a
website and don't need to support very old browsers.

</details>

<details>
<summary>What does the chroma subsampling setting actually do?</summary>

JPEG can store colour information at half resolution (`4:2:0`) because eyes are less sensitive to
colour detail than to brightness. That is fine for photographs and makes files noticeably smaller.
It is *not* fine for a sharp coloured logo or coloured text on a flat background, where it causes
visible fringing. `4:4:4` stores colour at full resolution. It only applies to JPEG output — PNG
is always full RGB, and WebP always writes 4:2:0.

</details>

<details>
<summary>Is my image uploaded anywhere?</summary>

No. The page runs a WebAssembly build of ffmpeg inside your browser tab, so the file is read from
your device, converted in memory, and offered back as a download. It is never sent to a server.

</details>

<details>
<summary>Can I convert a CMYK TIFF, PDF or PSD?</summary>

Not here — this tool accepts PNG, JPEG and WebP. Export or save your document as a JPEG first
(CMYK is fine, that's the point), then convert it here. A PDF or PSD is a document format rather
than a single image, so it needs a different kind of tool entirely.

</details>
