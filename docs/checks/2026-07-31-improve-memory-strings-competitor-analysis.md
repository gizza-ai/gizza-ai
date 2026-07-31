# memory-strings — competitor analysis (2026-07-31)

Tool function: run a `strings`-style extraction over a pasted memory / process
dump (raw text bytes, or a hex-encoded dump), pulling out the printable ASCII
**and** UTF-16LE runs, then categorize the results into the artifact classes an
analyst triages first — URLs, IPv4, IPv6, emails, domains, file paths (Windows,
UNC, and Unix), and Windows registry keys. Browser-local, no upload.

This is distinct from the existing `blocks/ioc-extract` (which refangs + regex-
matches *clean text* for IPs/URLs/domains/emails/hashes): memory-strings adds the
`strings`-utility step (printable-run extraction from raw/binary bytes, ASCII +
wide/UTF-16LE, a `min_length` floor, optional hex-dump decode) and the memory-
forensics-specific **file-path** and **registry-key** categories that ioc-extract
does not have. hashes are ioc-extract's job; memory-strings does not duplicate it.

## Top competitors surveyed

1. **InventiveHQ — String Extractor** (`inventivehq.com/tools/security/string-extractor`)
   Browser-side "extract text from binary files". Detects IPv4/IPv6, URLs, email
   addresses, file paths (Windows and UNC), registry keys, and Base64. Groups the
   output by category. Closest match to our target model.

2. **Splinter** (`github.com/pygrum/splinter`)
   A "weaponized strings" CLI. Extracts specific string *types* — URLs, IPs,
   registry keys, files, filetypes — supports a regex filter, a min-length, and
   exports **JSON with strings categorized by type**. Confirms category-grouped
   JSON output is a table-stake for the CLI/automation crowd.

3. **Themida-Dumper** (`github.com/nelj14/themida-dumper`)
   Process-memory IOC scanner. Notably extracts **both ASCII and UTF-16LE**
   strings and scans for registry keys, file paths, email addresses, file
   extensions, plus URLs (incl. onion) and wallet addresses. Validates the wide
   (UTF-16LE) extraction path as important for Windows memory.

4. **Pagefile.sys Parser** (`pagefilesysparser.com`)
   Free, private, **client-side** Windows pagefile forensic parser: extracts
   strings, finds URLs, registry keys, and IOCs entirely in the browser. Confirms
   the browser-local, no-upload positioning we already have.

5. **GNU `strings` / Eric Zimmerman `bstrings`** (classic CLI baseline)
   `strings -n <len>` sets the minimum run length; `-e l` / `-e b` select the
   encoding (7-bit ASCII, 16-bit little/big-endian). `bstrings` adds built-in
   regex presets (URL, IP, email, GUID, …). The `min_length` + `encoding`
   controls are the universal, expected knobs.

## Feature matrix (competitor → in our model?)

| Capability | Competitors | memory-strings |
|---|---|---|
| Printable ASCII string extraction | all | ✅ core |
| UTF-16LE (wide) string extraction | Themida, bstrings (`-e l`) | ✅ `encoding=utf16le/both` |
| Minimum run length (`-n`) | strings, bstrings, Splinter | ✅ `min_length` (default 4) |
| Hex-encoded dump input | (analysts pre-decode) | ✅ `input_format=hex` — decodes `48 65…` / `48:65` / `0x48` |
| URL category | all | ✅ |
| IPv4 / IPv6 categories | InventiveHQ, Splinter, strings-regex | ✅ |
| Email category | InventiveHQ, Themida | ✅ |
| Domain category | (implied via URLs) | ✅ (bare hosts, de-duplicated vs URL/email hosts) |
| Windows + UNC file paths | InventiveHQ, Splinter, Themida | ✅ `path` |
| Unix file paths | strings-on-linux-dumps | ✅ `path` |
| Registry keys | InventiveHQ, Splinter, Themida, Pagefile | ✅ `registry` |
| Category filter (subset) | Splinter (`-types`) | ✅ `categories` |
| Defang output for safe reports | (ioc tools) | ✅ `defang` (reuses our house convention) |
| Grouped, de-duplicated, counted output | InventiveHQ, Splinter | ✅ report + counts |
| Browser-local / no upload | Pagefile, InventiveHQ | ✅ |

## Considered, not built (out-of-model or deliberately scoped out)

- **Base64 / encoded-blob detection + decode.** InventiveHQ and String-Analyzer
  surface Base64. Detecting long base64 runs is feasible but noisy (false-positive
  heavy on random binary), and decoding is already covered by
  `blocks/extract-decode-base64` / `blocks/encoded-payload-decoder`. Left out to
  keep precision; noted here so it isn't silently dropped.
- **Hash extraction (md5/sha1/…).** Deliberately deferred to `blocks/ioc-extract`
  to avoid a semantic overlap; users who want hashes use that tool.
- **String offsets / file offsets.** `strings -t` prints byte offsets. Meaningful
  for a raw file, less so for pasted/hex text where the offset is ambiguous after
  extraction; skipped for now.
- **Binary FILE upload.** The gizza page model for a pure block is text-in; a real
  `.dmp`/`.raw` upload would need a file-input surface. Covered instead by the
  `input_format=hex` path (paste a hexdump) — no server, still browser-local.
- **Onion URLs / crypto-wallet / mutex categories** (Themida). Niche; the URL
  category already captures `.onion` hosts. Not added as separate buckets.
- **Cloud/API/batch, accounts, paid tiers** (various SaaS) — out of gizza's
  browser-local, no-account model.

## No-copy note

No competitor copy, branding, wording, or assets were reproduced. Categories and
option names are the standard forensics vocabulary (`strings -n`, registry hives,
UNC paths); all page copy, FAQ, and descriptor text is original.
