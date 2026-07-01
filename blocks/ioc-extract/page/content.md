## About this tool

The IOC extractor scans an arbitrary block of text and pulls out every **indicator of compromise** an analyst cares about, grouped by type and de-duplicated. Paste a firewall log, a phishing email, a SIEM alert or a vendor threat report and get a clean, sorted list of:

- **IPv4 and IPv6 addresses** — full, compressed (`::1`) and IPv4-mapped forms
- **URLs** — `http`, `https` and `ftp`
- **Domains / hostnames** — bare FQDNs (URL and email hosts are kept out of this group so categories don't overlap)
- **Email addresses**
- **File hashes** — MD5 (32 hex), SHA-1 (40 hex), SHA-256 (64 hex) and SHA-512 (128 hex)

### Handles defanged input

Indicators in threat reports are usually **defanged** so they can't be clicked by accident — `hxxp[://]evil[.]com`, `1[.]2[.]3[.]4`, `bad[at]evil[dot]com`. This tool refangs them automatically before matching (it understands the square `[]`, round `()` and curly `{}` bracket conventions plus the `[dot]`/`[at]`/`hxxp` variants), so you can paste straight out of a PDF or ticket.

### Re-defang the output

Tick **Re-defang the extracted indicators** to get the indicators back in defanged form (`evil[.]com`, `hxxp[://]…`, `bad[at]evil[.]com`) so the list itself is safe to drop into a report, ticket or email without auto-linking.

### Filter by type

Leave the type field as `all` to extract everything, or list just the categories you want — e.g. `ipv4,url,sha256`, or `hash` for all four hash types.

### Private by design

Everything runs locally in your browser via WebAssembly. Nothing you paste is uploaded to a server.

### FAQ

<details>
<summary>Which defanging styles does it understand?</summary>

Square, round, and curly bracket conventions — `evil[.]com`, `evil(.)com`, `evil{.}com` — plus `hxxp`/`hxxps`, `[dot]`, and `[at]`. Input is refanged before scanning, so defanged and clean indicators in the same paste are both found and merged into one de-duplicated list.

</details>

<details>
<summary>Why isn't the URL's domain also listed under "Domains"?</summary>

Categories are deliberately non-overlapping: a host that appears inside an extracted URL or email is kept out of the domain group. That way `hxxp[://]evil[.]com/payload` yields one URL, not a URL *and* a duplicate domain entry.

</details>

<details>
<summary>How do I extract only certain indicator types?</summary>

Set the type filter to a comma-separated list — e.g. `ipv4,url,sha256`. The shorthand `hash` selects all four hash types (MD5, SHA-1, SHA-256, SHA-512), and `all` (or leaving it empty) extracts everything.

</details>

<details>
<summary>How are the hash types told apart?</summary>

Purely by length: 32 hex characters is reported as MD5, 40 as SHA-1, 64 as SHA-256, and 128 as SHA-512. A truncated or oddly-delimited hash that doesn't hit one of those lengths won't match.

</details>
