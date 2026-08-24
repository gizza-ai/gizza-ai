## What this tool does

**Brotli Decompress** takes a Brotli-compressed payload — the codec behind HTTP
`content-encoding: br` and most modern JavaScript/CSS asset bundles — and shows
you the original data. Paste the blob as **Base64** or **hex**, and the
decompressed bytes appear as text, hex, or Base64. Decoding is pure Rust
compiled to WebAssembly and runs entirely in the page, so a payload copied out
of a production response or a customer bug report never leaves your machine.

## Worked example

Paste this Base64 payload with **Input encoding** on *Auto-detect* and **Show
result as** on *Text*:

```
GzYA6IzTFfMjhHUPwW2pr1bT4W8HRZOIyaLT1YpIQtBEajXzAUMiA3t2HQ3lRtbqeY1Rlg8=
```

You get the JSON that was compressed into it:

```json
{"user":"ada","roles":["admin","deploy"],"active":true}
```

Turn on **Show size and ratio stats** and the result is prefixed with a summary
before the payload:

```
Compressed:   53 bytes
Decompressed: 55 bytes
Ratio:        1.04x (decompressed / compressed)
Space saved:  3.6%
```

(Fifty-five bytes of JSON is far too small for Brotli to help — the preset chip
*High-ratio blob with stats* shows the same summary on a payload that compresses
30× instead.)

## Where Brotli payloads come from

- **HTTP response bodies.** Servers negotiate `Accept-Encoding: br` and return a
  Brotli-compressed body. Copying that body out of a proxy log or a `curl --raw`
  capture leaves you with bytes no editor can read.
- **Asset bundles.** Build pipelines ship pre-compressed `.br` files next to
  their `.js`/`.css` originals, and CDNs serve them directly.
- **Embedded blobs.** Config, telemetry, and cache entries are often Brotli'd and
  then Base64'd so they survive JSON or a URL.

## Encodings

**Input encoding** is *Auto-detect* by default. Auto-detection here is verified,
not guessed: a short Base64 string can consist entirely of hex characters, so
the tool decodes the payload both ways and keeps whichever one actually
Brotli-decompresses. Force **Base64** or **Hex** when you already know the form
and want a precise error instead of a fallback.

- Base64 accepts the standard and URL-safe alphabets, with or without `=`
  padding.
- Hex ignores an optional `0x` prefix and is case-insensitive.
- Both ignore ASCII whitespace and line breaks, so a wrapped paste works
  unchanged.

**Show result as** defaults to *Text* (UTF-8). Switch to **Hex** or **Base64**
when the payload decompresses to binary — a font, an image, a protobuf — instead
of readable text.

## Limits and edge cases

- **8 MiB** of compressed input and **16 MiB** of decompressed output. The
  payload is held in WebAssembly memory, so an unbounded blob would exhaust the
  tab; past these caps you get a clear error rather than a crash. Large `.br`
  files are better handled as a file download by the *file-compressor* tool
  (`operation=decompress`, `format=brotli`).
- **Brotli has no magic number.** Nothing about the first few bytes proves a blob
  is Brotli, which is why this tool never rejects input up front. If the decode
  fails and the bytes carry a *different* codec's signature, the error names that
  codec and the tool that handles it — gzip → *gunzip*, zlib → *raw-inflate*,
  xz or raw LZMA → *lzma-decompress*, LZ4 → *lz4-decompress*, zstd →
  *file-compressor*, bzip2 or tar → *archive-extractor*, ZIP → *unzip*, 7-Zip →
  *7z-extract*.
- **A truncated payload cannot be recovered.** Brotli decoding is sequential, so
  half a stream produces a decode error, not half the text. Copy the whole
  compressed body.
- **An empty payload is valid.** Brotli can encode zero bytes, and the result is
  an empty output rather than an error.
- Text output rejects non-UTF-8 bytes on purpose — mojibake hides real data, so
  the error tells you to switch to hex or Base64 instead.

## FAQ

<details>
<summary>What input formats can I paste?</summary>

Base64 or hex. Base64 accepts both the standard (`+/`) and URL-safe (`-_`) alphabets, padded or not. Hex accepts an optional `0x` prefix in any case. Both ignore whitespace and line breaks, so a payload wrapped across many lines pastes fine. Leave **Input encoding** on *Auto-detect* and the tool works out which one you used.

</details>

<details>
<summary>Can I upload a .br file directly?</summary>

Not on this page — it is a paste-in, read-out tool for inspecting payloads inline. For a `.br` file you already have on disk or at a URL, use the *file-compressor* tool with `operation=decompress` and `format=brotli`, which returns the decompressed file as a download.

</details>

<details>
<summary>Why does it say my payload is gzip (or zstd, or ZIP) and not Brotli?</summary>

Because the Brotli decode failed and the first bytes matched another codec's signature. Brotli itself has no magic number, so that check only runs *after* the decode attempt — a valid Brotli payload is never misidentified. The message names the codec found and the sibling tool that decodes it.

</details>

<details>
<summary>Why is my result unreadable, or why do I get a UTF-8 error?</summary>

The payload decompressed to binary rather than text. Switch **Show result as** to *Hex* or *Base64* to see the raw bytes. Text output deliberately errors instead of printing replacement characters, because silently mangled output is worse than a clear message.

</details>

<details>
<summary>How large a payload can it handle?</summary>

Up to 8 MiB compressed and 16 MiB decompressed. Both limits exist because the data lives in WebAssembly memory inside the tab; exceeding either returns an explicit error naming the limit. Typical HTTP bodies and bundle chunks are far below it.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The Brotli decoder is compiled to WebAssembly and runs inside this page, so the payload stays in your browser — which matters when it came from a production response, a customer report, or a security investigation.

</details>
