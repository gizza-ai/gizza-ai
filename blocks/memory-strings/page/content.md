## About this tool

**Memory Strings Extractor** runs a `strings`-style pass over a memory or process
dump and then does the triage step for you: it recovers every printable run and
sorts the results into the artifact classes a DFIR or malware analyst looks at
first. Paste a dump as raw text, or as a hex-encoded dump (`48 65 6c 6c 6f`,
`0x48`, `48:65`, or contiguous hex) when you only have a hexdump to hand.

Two things happen locally in your browser:

1. **Extract** — printable runs of at least *min length* characters are pulled
   out, treating non-printable bytes as delimiters, exactly like `strings -n`.
   You can recover 7-bit **ASCII** runs, **UTF-16LE** (wide) runs — the common
   encoding for text in Windows memory — or both.
2. **Categorize** — the recovered strings are grouped, de-duplicated, sorted and
   counted into **URLs**, **IPv4** and **IPv6** addresses, **emails**, bare
   **domains**, **file paths** (Windows drive, UNC and Unix) and Windows
   **registry keys**.

Turn on **defang** to rewrite the URLs, IPs, domains and emails as
`hxxp[://]evil[.]com` so the result is safe to drop into a ticket or report, and
use **categories** to report just the buckets you care about (for example
`registry` or `url,ipv4`).

Everything runs in your browser with WebAssembly — the dump is never uploaded.

### Worked example

Paste this dump with **Categories = all**:

```
GET http://evil.example.com/a from 203.0.113.5 open C:\Windows\System32\cmd.exe key HKLM\Software\Microsoft\Windows\CurrentVersion\Run mail bad@phish.net host cdn.badsite.io
```

You get grouped output like:

```
URLs (1):
  http://evil.example.com/a
IPv4 addresses (1):
  203.0.113.5
Emails (1):
  bad@phish.net
Domains (1):
  cdn.badsite.io
File paths (1):
  C:\Windows\System32\cmd.exe
Registry keys (1):
  HKLM\Software\Microsoft\Windows\CurrentVersion\Run
```

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>What is the difference between ASCII and UTF-16LE strings?</summary>

Classic `strings` finds **ASCII** runs — sequences of printable 7-bit bytes.
Windows stores much of its text as **UTF-16LE** (each character followed by a
`0x00` byte), so a plain ASCII scan misses it. Set **encoding** to `both`
(the default) to recover ASCII and wide runs, or pick one when you know the
source. This mirrors `strings -e l` for the wide path.

</details>

<details>
<summary>My dump is binary — how do I paste it?</summary>

Text fields can't carry raw non-printable bytes, so paste a **hex-encoded
dump** and set **Input format** to `hex`. The decoder accepts contiguous hex,
bytes separated by spaces, colons or commas, and optional `0x` prefixes — e.g.
`48 65 6c 6c 6f`, `48:65:6c`, or `0x48 0x65`. This keeps the null bytes that a
UTF-16LE scan needs, which pasting the raw text would strip.

</details>

<details>
<summary>Why do I see fewer or more strings when I change min length?</summary>

**Minimum run length** is the `strings -n` floor: runs shorter than it are
treated as noise and dropped. The default is `4`. Lower it (down to `1`) to
catch short tokens at the cost of more junk, or raise it to keep only longer,
more meaningful strings. The count in the header reflects how many runs survived
the floor.

</details>

<details>
<summary>Does this extract file hashes too?</summary>

No — hash extraction (MD5/SHA-1/SHA-256/…) is intentionally left to the separate
**IOC Extractor** tool so the two don't overlap. This tool focuses on the
`strings`-style recovery plus the memory-forensics categories (file paths and
registry keys) that a hash extractor doesn't cover. Run both if you need hashes
as well.

</details>

<details>
<summary>Is my dump uploaded anywhere?</summary>

No. The extraction and categorization run entirely in your browser via
WebAssembly. The dump you paste never leaves your machine — there is no server
call, no account and no logging.

</details>
