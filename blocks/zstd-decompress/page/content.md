## What this tool does

**Zstandard Decompress** takes a Zstandard-compressed payload — the codec behind HTTP `content-encoding: zstd`, `.zst` blobs, Kafka records, and ClickHouse exports — and shows you the original data. Paste the compressed bytes as **Base64** or **hex**, and the decompressed result appears as text, hex, or Base64. Decoding is pure Rust compiled to WebAssembly and runs entirely in the page, so a payload copied out of a production response or a customer report never leaves your machine.

Unlike older pasted-payload decoders, this one understands the zstd stream structure. It can decode concatenated multi-frame streams, step over legal skippable metadata frames, report frame headers, and verify the trailing xxHash-32 content checksum when a frame carries one.

## Worked example

Paste this Base64 payload with **Input encoding** on *Auto-detect* and **Show result as** on *Text*:

```
KLUv/SQ3nQEAcsMLEaBtDNw9rldW0gajapZ9/r8hcx9z4YXGER2Mozpry+BkUNFFuO1RbLAWPcy2vWUA22iyIw==
```

You get the JSON that was compressed into it:

```json
{"user":"ada","roles":["admin","deploy"],"active":true}
```

Turn on **Show size and ratio stats** and the result is prefixed with a summary before the payload:

```
Compressed:   64 bytes
Decompressed: 55 bytes
Ratio:        0.86x (decompressed / compressed)
Space saved:  -16.4%
Frames:       1 data frame
```

Small JSON can grow after compression because the zstd frame header and checksum cost more than the repeated data saves. The preset chip *High-ratio blob with stats* shows the same summary on a repetitive payload that compresses far better.

## Where zstd payloads come from

- **HTTP response bodies.** Servers can negotiate `Accept-Encoding: zstd` and return a compressed body. Copying that raw body out of a proxy log or a `curl --raw` capture leaves you with bytes no editor can read.
- **Data systems.** Kafka messages, ClickHouse exports, and cache entries often compress a record with zstd and then Base64 it so it survives JSON, logs, or URLs.
- **Package and build artifacts.** `.zst` chunks appear in package repositories, container layers, and build caches. For a real file workflow, use the file-oriented compressor/decompressor instead.

## Encodings

**Input encoding** is *Auto-detect* by default. Auto-detection here is verified, not guessed: zstd data frames begin with the magic bytes `28 b5 2f fd`, so the tool decodes the paste as both hex and Base64 and keeps whichever one yields zstd bytes. Force **Base64** or **Hex** when you already know the form and want a precise input error.

- Base64 accepts the standard and URL-safe alphabets, with or without `=` padding.
- Hex ignores an optional `0x` prefix and is case-insensitive.
- Both ignore ASCII whitespace and line breaks, so a wrapped paste works unchanged.

**Show result as** defaults to *Text* (UTF-8). Switch to **Hex** or **Base64** when the payload decompresses to binary — a protobuf, a thumbnail, a packed index — instead of readable text.

## Frame details

Turn on **Show zstd frame details** when you are debugging a stream rather than just reading its payload. The report names each data frame, its compressed and decompressed size, the decoder window size, the declared content size, any dictionary ID, and whether the content checksum was present and verified. Legal skippable frames are reported with their magic number and skipped payload size.

That matters for streams produced by parallel compressors or `zstd -c a b > out.zst`: they can contain multiple data frames in one byte stream. A one-shot decoder can stop at the first frame and silently return partial data. This tool keeps decoding until the stream is actually exhausted.

## Limits and edge cases

- **8 MiB** of compressed input and **16 MiB** of decompressed output. The payload is held in WebAssembly memory, so an unbounded blob would exhaust the tab; past these caps you get a clear error rather than a crash. Large `.zst` files are better handled as a file download by the *file-compressor* tool (`operation=decompress`, `format=zstd`).
- **Dictionary-compressed frames need the original dictionary.** If a frame declares a dictionary ID, the tool errors and prints that ID instead of pretending it can decode without the dictionary file.
- **Wrong-codec blobs are rejected up front.** Because zstd has a magic number, gzip, zlib, xz, raw LZMA, LZ4, bzip2, ZIP, 7-Zip, and tar inputs are named with the sibling tool that handles them.
- **A truncated payload cannot be recovered.** Zstd decoding is sequential, so half a stream produces a decode error, not half the text. Copy the whole compressed body.
- Text output rejects non-UTF-8 bytes on purpose — mojibake hides real data, so the error tells you to switch to hex or Base64 instead.

## FAQ

<details>
<summary>What input formats can I paste?</summary>

Base64 or hex. Base64 accepts both the standard (`+/`) and URL-safe (`-_`) alphabets, padded or not. Hex accepts an optional `0x` prefix in any case. Both ignore whitespace and line breaks, so a payload wrapped across many lines pastes fine. Leave **Input encoding** on *Auto-detect* and the tool uses the zstd magic number to work out which one you used.

</details>

<details>
<summary>Can I upload a .zst file directly?</summary>

Not on this page — it is a paste-in, read-out tool for inspecting inline payloads. For a `.zst` file you already have on disk or at a URL, use the *file-compressor* tool with `operation=decompress` and `format=zstd`, which returns the decompressed file as a download.

</details>

<details>
<summary>Why does it mention skippable frames?</summary>

Zstd reserves a range of frame magic numbers for application metadata. Those frames are legal inside a zstd stream but do not produce decompressed bytes. The tool skips them, reports them when **Show zstd frame details** is enabled, and keeps decoding the next real data frame.

</details>

<details>
<summary>Why does it say my payload is gzip, ZIP, or another format?</summary>

The first decoded bytes matched another file or compression signature instead of the zstd magic number. The error names the likely codec and the sibling tool that handles it so you do not waste time tweaking Base64 or hex settings on the wrong data.

</details>

<details>
<summary>Why is my result unreadable, or why do I get a UTF-8 error?</summary>

The payload decompressed to binary rather than text. Switch **Show result as** to *Hex* or *Base64* to see the raw bytes. Text output deliberately errors instead of printing replacement characters, because silently mangled output is worse than a clear message.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The zstd decoder is compiled to WebAssembly and runs inside this page, so the payload stays in your browser — which matters when it came from a production response, a customer report, or a security investigation.

</details>
