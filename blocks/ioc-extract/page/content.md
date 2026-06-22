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
