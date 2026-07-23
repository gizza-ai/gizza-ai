## About this tool

This recipe pipeline lets you paste data and run it through a chain of byte-level decode and transform steps — Base64, hex, URL percent-encoding, gzip/zlib/raw DEFLATE, repeating-key XOR, ROT13, and simple byte arithmetic — applied one after another. It is a focused, page-friendly take on the recipe model made popular by tools like CyberChef, and it runs entirely as WebAssembly in your browser, so nothing you paste is ever uploaded.

Write one operation per line in the **Recipe** box. Operations run from top to bottom over a single byte buffer, so you can unwrap layered payloads in one shot. For example, this recipe:

```text
url-decode
from-base64
```

applied to this input:

```text
SGVsbG8sIHdvcmxkIQ%3D%3D
```

produces:

```text
Hello, world!
```

The `%3D%3D` is first percent-decoded to `==`, then the resulting `SGVsbG8sIHdvcmxkIQ==` is Base64-decoded. A classic malware-string unwrap looks like `from-base64` → `gunzip` → `xor 2a`.

Supported operations:

- `from-base64` / `to-base64` — Base64 decode (tolerant of whitespace, the URL-safe alphabet, and missing padding) and standard padded encode.
- `from-hex` / `to-hex` — hex decode (ignores whitespace, `:`, `,`, and a `0x` prefix) and lowercase encode.
- `url-decode` / `url-encode` — percent-decode `%XX` and percent-encode non-alphanumeric bytes.
- `gunzip` / `gzip`, `zlib-inflate` / `zlib-deflate`, `raw-inflate` / `raw-deflate` — decompress and compress with gzip, zlib (RFC 1950), or raw DEFLATE (RFC 1951).
- `xor KEY [hex|utf8|base64|decimal]` — repeating-key XOR; the key format defaults to hex, e.g. `xor 2a` or `xor secret utf8`.
- `add N` / `sub N` — add or subtract a byte (mod 256), `N` decimal or `0x..`; `not` bitwise-NOTs every byte.
- `reverse`, `upper`, `lower` — reverse the byte order, or ASCII upper/lowercase.

Blank lines and lines starting with `#` are ignored, so you can comment and space out a recipe. The **Output rendering** control decides how the final bytes are shown: **Auto** displays UTF-8 text when the whole result is printable and falls back to hex otherwise, while **Hex** and **Base64** force those encodings for binary results.

### Limits

The recipe covers the common decode/transform chains, not the full 300+ operation catalog of a desktop toolkit. Keyed modern crypto (AES, DES, ChaCha20…), hashing, and checksums are intentionally left to dedicated single-purpose tools; only XOR and byte math — the usual last obfuscation layer — are included here. The working buffer is capped at 16 MB at every step to guard against decompression bombs, input is treated as UTF-8 bytes, and there is no drag-and-drop visual builder — the recipe is a plain text list of operations.

## FAQ

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The entire pipeline runs locally as WebAssembly inside your browser tab. The data you paste never leaves your device — there is no server round-trip, so it is safe for sensitive payloads and offline use.

</details>

<details>
<summary>How do I decode a From Base64 → Gunzip → XOR payload?</summary>

Put each step on its own line in the order they should be undone: `from-base64`, then `gunzip`, then `xor 2a` (replace `2a` with the real key). This reverses the common "encode then compress then XOR-obfuscate" wrapping used to hide strings. If you do not know the XOR key, try single-byte values or use a dedicated XOR-brute-force tool.

</details>

<details>
<summary>What key formats does the XOR step accept?</summary>

Write `xor KEY` followed by an optional format token: `hex` (default, e.g. `xor 2a` or `xor deadbeef`), `utf8` for a text key (`xor secret utf8`), `base64`, or `decimal` for comma/space-separated byte values (`xor 65,66 decimal`). The key repeats across the buffer, and because XOR is symmetric the same step both encrypts and decrypts.

</details>

<details>
<summary>My result shows hex instead of text — why?</summary>

With **Auto** rendering, the output is shown as text only when the whole byte buffer is valid UTF-8; otherwise it falls back to lowercase hex so binary bytes stay visible. If you expected text, a step probably produced raw bytes (for example a partial decompression or a wrong XOR key). Switch **Output rendering** to Hex or Base64 to inspect the bytes, or fix the recipe.

</details>

<details>
<summary>Can it run AES, hashes, or arbitrary code?</summary>

No. This tool is a fixed set of decode/transform operations and never evaluates user code. Keyed modern ciphers (AES, DES, Blowfish, RC4, ChaCha20) and hash/checksum functions live in dedicated tools; XOR and byte arithmetic are included here because they are the common lightweight obfuscation layer in decode chains and need no key management.

</details>
