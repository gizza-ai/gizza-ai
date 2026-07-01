## What this tool does

**LZNT1 Decompress** decodes a blob compressed with Windows' built-in LZNT1
algorithm — the format emitted by `RtlCompressBuffer` /
`RtlDecompressBuffer` when called with `COMPRESSION_FORMAT_LZNT1`. Paste the
compressed bytes as **hex** or **Base64**, pick how you want the recovered data
shown (hex, plain UTF-8 text, or Base64), and the original bytes appear
instantly. Everything runs locally in your browser — the blob never leaves your
machine.

## Where LZNT1 shows up

LZNT1 is the legacy compression scheme baked into Windows, so it turns up
constantly in systems and security work:

- **NTFS compressed files** — files marked "compressed" in Explorer are stored
  as LZNT1 chunks on disk.
- **Registry hives** — many hive cells and `RegLoadAppKey`-style blobs are
  LZNT1-compressed.
- **Hibernation files** — `hiberfil.sys` pages are LZNT1-compressed when written.
- **Malware configuration** — a large number of malware families compress their
  embedded C2 / config data with `RtlCompressBuffer`, so reversers routinely need
  to inflate LZNT1 blobs pulled out of a sample.

## How LZNT1 works

An LZNT1 stream is a sequence of **chunks**. Each chunk begins with a 16-bit
little-endian header: the top bit flags whether the chunk body is compressed
(otherwise it is stored verbatim), and the low 12 bits hold the body length
minus one. A compressed body is split into **flag groups** — one flag byte
followed by up to eight tokens. Each bit of the flag byte (least-significant
first) marks the matching token as either a single literal byte or a 16-bit
**back-reference**. The split between a back-reference's length and displacement
fields shifts as the window fills, which is what makes LZNT1 trickier to decode
than a plain LZ77 stream. This tool implements that wire format directly, so it
needs no Windows API and works on any platform.

## Tips

- Output defaults to **hex** because decompressed blobs are usually binary; switch
  to **text** when you expect readable UTF-8 (e.g. a JSON or string config).
- Hex input is forgiving: whitespace and an optional `0x` prefix are ignored.
- If decompression fails, double-check that the blob is genuinely LZNT1 (not raw
  GZIP/zlib/LZ4) and that you copied the *entire* compressed buffer.

## FAQ

<details>
<summary>What input formats are accepted?</summary>

Hex (the default) or Base64. Hex is forgiving — whitespace, line breaks, and an optional `0x` prefix are ignored, and case doesn't matter. Base64 works with or without `=` padding. Output can be hex, UTF-8 text, or Base64.

</details>

<details>
<summary>Why do I get a "truncated LZNT1 stream/chunk" error?</summary>

The chunk header declares how many body bytes follow; if fewer remain (or a back-reference token is missing its second byte), the blob was cut short. Usually you copied only part of the buffer — grab the entire compressed region and try again.

</details>

<details>
<summary>Can it decompress LZNT1 data that Windows stored uncompressed?</summary>

Yes. `RtlCompressBuffer` emits verbatim (stored) chunks when compression wouldn't help; the chunk header's top bit marks those, and the tool copies them through unchanged. A stream mixing compressed and stored chunks decodes correctly.

</details>

<details>
<summary>Does this handle Xpress or Xpress Huffman blobs?</summary>

No — only `COMPRESSION_FORMAT_LZNT1`. Newer Windows features (Win10 memory compression, some hibernation formats) use LZ77/Xpress or Xpress Huffman, which are different wire formats; an LZNT1 decoder will error on them.

</details>

<details>
<summary>Is my blob uploaded anywhere?</summary>

No — decoding is pure Rust compiled to WebAssembly and runs entirely in the page, which matters when the blob comes from a sensitive forensic image or malware sample.

</details>
