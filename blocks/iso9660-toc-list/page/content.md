# ISO 9660 TOC List

Paste the **base64 text** for a `.iso` disc image and get a read-only table of contents: the
volume label, directory tree, file sizes and a quick summary. The parser reads only ISO 9660 volume
descriptors and directory records; it never extracts or decodes the file contents stored in the
image.

## What it reads

- The Primary Volume Descriptor at sector 16 (`CD001`).
- The volume identifier / label.
- The root directory record and nested directory extents.
- File identifiers and byte sizes, with ISO `;1` version suffixes stripped.
- Joliet supplementary descriptors when present, so long mixed-case UCS-2 filenames are preferred.

Choose **tree** for an indented directory tree, **list** for one path per line, or **summary** for
label, standard, block size and counts. To prepare input locally, run `base64 disc.iso` and paste the
output.

## Worked example

A small ISO with volume label `TESTDISC`, a root file `README.TXT` (11 bytes), and a `DOCS/A.TXT`
file (4 bytes) renders like:

```text
/  (volume: TESTDISC)
├── DOCS/
│   └── A.TXT  (4 B)
└── README.TXT  (11 B)
```

## Limits and edge cases

- Input must be base64 text for an ISO 9660 image; raw binary bytes are not accepted in the text box.
- The parser is metadata-only: it does not extract file contents or validate checksums.
- Directory traversal is cycle-guarded and capped to avoid pathological images.
- Rock Ridge POSIX extensions are not interpreted; Joliet long filenames are preferred when present.
- Very large pasted base64 strings may be uncomfortable in a browser text field. Use the CLI/chat
  surface for larger images if available.

## FAQ

<details>
<summary>Does this extract files from the ISO?</summary>

No. It only reads the volume descriptors and directory records needed to list names and sizes. File
payloads are not copied, decompressed or written anywhere.

</details>

<details>
<summary>Why does the input need base64?</summary>

This tool is a text-field page, so the binary `.iso` bytes must be represented as text. Run
`base64 disc.iso` locally, paste the result, and choose the output format. Whitespace in the base64
is tolerated.

</details>

<details>
<summary>Does it support Joliet long filenames?</summary>

Yes. When a Joliet supplementary volume descriptor is present, the parser prefers its UCS-2 names;
otherwise it falls back to standard ISO 9660 identifiers and strips the usual `;1` version suffix.

</details>

<details>
<summary>Can it show Rock Ridge permissions or symlinks?</summary>

No. It lists the portable ISO 9660/Joliet table of contents: names, directories, sizes and the
volume label. Rock Ridge POSIX metadata is out of scope for this pass.

</details>
