## About this tool

QR Paper Backup turns text or pasted file bytes into a printable SVG sheet of numbered QR codes. It is meant for offline archival of secrets, recovery notes, tiny configuration files, license keys, or other small payloads that you want to store on paper rather than another drive.

Each QR code contains a self-describing line:

```text
QRB1|<index>|<total>|<id>|<base64-chunk>
```

The `id` is the first 8 hex characters of the SHA-256 digest of the complete original data, so every part can be tied back to the same set. To restore, scan all parts, sort them by index, concatenate the Base64 chunk fields, then Base64-decode the result.

## Worked example

Input:

```text
paper backup demo
```

Use **Input encoding = Text**, **Bytes per QR code = 300**, **Columns = 2**, **QR error correction = M**, and keep **Print payload text** on. The output is a deterministic SVG sheet with one QR code, a printed restore header, and the exact `QRB1|1|1|...` payload line under the code.

## Limits & edge cases

- This tool creates the backup sheet only; it does not scan photos or restore a backup from images.
- Paste file bytes as Base64 or hex. The page does not read local folders or upload files.
- `chunk_bytes` is clamped to 50–1200 raw bytes. Smaller chunks are easier to scan; larger chunks produce fewer QR codes.
- QR error correction protects each individual code. There is no erasure coding across missing pages, so print and store every part.
- The output is SVG. Use your browser's print dialog to print it or save it as PDF.

## FAQ

<details>
<summary>Can this encrypt my backup before printing?</summary>

No. Encrypt the data first with a separate encryption tool, then paste the ciphertext or its Base64 bytes here. Keeping encryption separate makes the printed QR format simple and auditable.

</details>

<details>
<summary>How do I restore the data later?</summary>

Scan every QR code, sort the lines by the numeric index field, concatenate the final Base64 chunk fields, and Base64-decode the combined text. The printed header repeats this process on the sheet.

</details>

<details>
<summary>What should I choose for bytes per QR code?</summary>

Use smaller chunks such as 100–300 bytes for reliable phone scanning on ordinary printers. Use larger chunks only when you have good print quality and want fewer codes.

</details>

<details>
<summary>Does losing one QR code make the backup unrecoverable?</summary>

Yes. QR error correction handles damage inside a code, but this tool does not add cross-code redundancy. Print all pages clearly and consider making multiple copies.

</details>
