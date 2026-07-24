## About this tool

The **Hash IOC Matcher** hashes a piece of file content and checks whether it is
a **known indicator of compromise** — a file already listed in a blocklist of
known-bad hashes. Everything runs locally in WebAssembly, so your sample and
your blocklist are **never uploaded** to a server, which makes it safe for
malware triage and sensitive investigations.

Threat-intel feeds and antivirus vendors publish **file hashes** of known
malware so defenders can recognise a bad file without ever executing it. This
tool computes the four digests those feeds use — **MD5, SHA-1, SHA-256 and
SHA-512** — and looks each one up in the blocklist you paste. If any digest
matches, the file is **FLAGGED**; if none do (or the blocklist is empty), it is
**CLEAN**.

### How it works

1. Paste the **file content** into the first box. If it is raw bytes rather than
   text, set **Interpret input as** to `hex` or `base64` so the real bytes are
   hashed.
2. Paste your **known-bad hash blocklist** into the second box — one hash per
   line, or any messy format you have.
3. Read the **result**: a FLAGGED/CLEAN status, the matched digest(s), the input
   size in bytes, how many blocklist entries were parsed, and every computed
   hash (with a `<-- MATCH` marker on the ones that hit).

### Blocklist parsing

The blocklist box is deliberately forgiving — paste feeds as they come. Every
maximal run of hex characters is scanned and kept only when its length matches a
digest width (**32, 40, 64 or 128** hex characters for MD5 / SHA-1 / SHA-256 /
SHA-512). That means all of these lines work, case-insensitively, and duplicates
collapse automatically:

- `900150983cd24fb0d6963f7d28e17f72`
- `MD5: 900150983CD24FB0D6963F7D28E17F72 , dropper.exe` (labelled + CSV)
- `sha256=0xBA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD` (`0x`-prefixed)
- `# a comment line` and `; note` (ignored)

Short hex runs that are not a digest width (e.g. `deadbeef`) are simply skipped,
so stray hex in filenames or notes will not create false matches.

### Worked example

Paste `abc` as the file content and this blocklist:

```
MD5: 900150983cd24fb0d6963f7d28e17f72 , dropper.exe
# unrelated sha256 from another feed
ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
```

The MD5 of `abc` is `900150983cd24fb0d6963f7d28e17f72`, so the result is
**FLAGGED** on `md5`, reports **2** blocklist entries parsed, and marks the MD5
row with `<-- MATCH`. Change the content to `hello` and the same blocklist now
returns **CLEAN** — none of `hello`'s digests appear in it.

### Limits

- This is a **hash lookup, not a scanner**. It cannot detect malware that is not
  already in your blocklist, and it does not fetch any live feed — you supply
  the known-bad hashes yourself.
- Only the four IOC-feed digest widths are recognised (MD5, SHA-1, SHA-256,
  SHA-512). SHA-224/384, SHA-3 and BLAKE hashes in a blocklist are ignored.
- An **MD5 or SHA-1** match proves the bytes are identical to a known-bad
  sample, but those algorithms are cryptographically broken — treat a
  SHA-256/512 match as the stronger signal.
- Because it runs in your browser, the whole file must fit in the text box;
  paste content, not multi-gigabyte binaries.

### Related tools

- To simply **compute** a file's hashes, use the **Hash Generator**.
- To **pull hashes and other IOCs out** of a threat report, use the **IOC
  Extractor**.
- To check content against a **single expected** checksum, use the **Checksum
  Verifier**.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details>. -->

<details>
<summary>Which hash algorithms are matched?</summary>

The tool computes **MD5, SHA-1, SHA-256 and SHA-512** for your input — the four
digest families threat-intel IOC feeds and antivirus vendors publish. Each one
is looked up in your blocklist independently, so a feed that only lists SHA-256
hashes still flags the file as long as its SHA-256 is present. Other widths
(SHA-224/384, SHA-3, BLAKE) are not computed and are ignored if they appear in
the blocklist.

</details>

<details>
<summary>What formats can the blocklist be in?</summary>

Almost anything. The parser scans for hex runs of digest length (32/40/64/128
characters) and keeps them, so plain one-per-line lists, `MD5: <hash>` labels,
CSV rows like `<hash>,malware.exe`, `0x`-prefixed values, and `#`/`;` comment
lines all work. Matching is case-insensitive and duplicate hashes collapse to a
single entry, which the result reports as the number of blocklist entries
parsed.

</details>

<details>
<summary>My file is a binary — how do I hash the real bytes?</summary>

Set **Interpret input as** to `hex` or `base64` and paste the file's bytes in
that encoding. The tool decodes them back to the raw bytes **before** hashing,
so the digests match what a normal file hash would produce. Leaving it on
`text` hashes the UTF-8 bytes of whatever characters you paste, which is only
correct for genuinely textual content.

</details>

<details>
<summary>Is my sample or blocklist uploaded anywhere?</summary>

No. All hashing and matching happen locally in your browser via WebAssembly.
Nothing you paste — the file content or the blocklist — leaves your machine, so
it is safe to use for malware triage and confidential investigations even on an
air-gapped host.

</details>

<details>
<summary>Does a CLEAN result mean the file is safe?</summary>

Only that it is **not in the blocklist you provided**. This tool is a hash
lookup, not a malware scanner — it cannot recognise a threat that is absent from
your list, nor detect novel or repacked samples whose hashes differ. Treat CLEAN
as "no known-bad hash matched," and pair it with a real scanner and up-to-date
feeds for a verdict.

</details>
