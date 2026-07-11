## Decode unknown base-encoded text

Paste an encoded blob and this tool tries the common base encodings — Base16
(hex), Base32, Base45, Base58, Base64 (standard and URL-safe), and Base85 — then
keeps peeling if the decoded result is another readable encoded layer. The report
shows the detected chain and the final text, or a binary signature plus a hex
preview when the bytes look like a file. Everything runs locally in your browser.

### Worked example

Input:

```
U0dWc2JHOHNJRmR2Y214a0lRPT0=
```

Output:

```
Detected 2 layer(s): base64 → base64

Hello, World!
```

The first Base64 decode produces another Base64 string, so the decoder peels one
more layer. Use **Output style = plain** when you want only the final decoded
value without the layer report.

### Options

- **Maximum layers to peel** — stops recursive decoding after this many accepted
  layers. The default is 8 and values are clamped to 1–30.
- **Output style** — `report` explains what was detected; `plain` returns only
  the final text, or hex when the final bytes are binary.

### Limits & edge cases

- Inputs are capped at 1 MB of encoded text.
- Auto-detection is heuristic: ambiguous strings are tried in a fixed order and
  only accepted when the decoded bytes are mostly printable text or match a known
  binary signature.
- For a single known scheme where every option matters, use the dedicated codec
  tools (Base32, Base58, Base64, Base85, hex) instead.
- Encrypted or compressed data is not decrypted/decompressed; this only removes
  base-encoding layers.

<details>
<summary>Which encodings are detected?</summary>

Base16/hex, Base32, Base45, Base58, Base64 (standard and URL-safe, padded or
unpadded), and Ascii85/Base85. Whitespace is ignored for most bases, so copied
wrapped lines usually work.

</details>

<details>
<summary>Can it decode several nested layers?</summary>

Yes. After each successful decode, the result is tested again as text. If it
looks like another supported base encoding, the next layer is peeled until the
depth cap, plaintext, or a recognized binary signature is reached.

</details>

<details>
<summary>What happens when the decoded bytes are binary?</summary>

If the output starts with a known magic byte signature (for example PNG, JPEG,
PDF, ZIP, gzip, ELF, or MP3), the report names that signature and shows a hex
preview. Plain mode returns the final bytes as hex.

</details>

<details>
<summary>Why did it leave my input unchanged?</summary>

The text may already be plaintext, may use an unsupported alphabet, may be too
short to distinguish safely, or may decode into non-printable bytes without a
known file signature. The tool avoids guessing when the candidate does not look
like a real decoded layer.

</details>
