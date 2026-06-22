## What this tool does

Paste any code or text and instantly see how big it would be **after gzip and
brotli compression** — the size it actually weighs when a server sends it with
`Content-Encoding: gzip` or `Content-Encoding: br`. You get the raw size, the
gzipped size, the bytes saved, the percent reduction, the compression ratio, and
a brotli comparison line.

Everything runs locally in your browser. Nothing is uploaded, it works offline,
and there is no sign-up.

## Why gzipped size matters

The number that affects your page load time is the **transfer size** — the
compressed bytes that travel over the network — not the raw file size on disk.
Almost every web server and CDN serves text assets (HTML, CSS, JavaScript, JSON,
SVG) with gzip or Brotli, so a 300 KB JavaScript file might only cost ~80 KB on
the wire. This tool shows you that compressed number so you can judge a bundle's
real weight, compare two versions, or check whether a minification or refactor
actually pays off after compression.

## How to read the report

| Field | Meaning |
| --- | --- |
| **Raw size** | The uncompressed UTF-8 byte length of your input. |
| **Gzipped size** | The size of the gzip stream (header + deflate body + trailer) at the chosen level. |
| **Saved** | Raw minus gzipped. For very short inputs this can be *negative* — gzip's ~18-byte header and trailer outweigh any savings. |
| **Reduction** | Bytes saved as a percentage of the raw size. |
| **Compression ratio** | Raw ÷ gzipped (e.g. `4.00x` means it shrank to a quarter). |
| **Brotli size** | The same input compressed with brotli at quality 11 (`Content-Encoding: br`), with its own percent reduction — usually a bit smaller than gzip on text. |

## gzip level

The **gzip level** (0–9) trades CPU for size. Level 6 is the default used by
gzip, zlib, nginx, and most servers — it's the realistic number for production.
Level 9 squeezes a little more out at extra CPU cost; level 0 stores the data
with no compression. Set the level to match your server if you want an exact
estimate.

## Notes

- Sizes use binary units (1 KB = 1024 bytes), matching browser dev tools.
- gzip estimates the wire size for `Content-Encoding: gzip`, the universally
  supported baseline; the brotli line estimates `Content-Encoding: br`, which
  every modern browser supports and which usually compresses text a bit smaller.
- Measurement is byte-exact for the gzip format — the same deflate engine used
  by real servers — so the number reflects what your input would actually weigh.
