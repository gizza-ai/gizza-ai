## About this tool

**Extract Domains** scans a block of text and pulls out every domain it
references — whether it appears as a bare hostname, inside an **http/https/ftp
URL**, or as part of an **email address**. Choose what you want back:

- **Hostnames** — the full host as written, e.g. `www.blog.example.co.uk`.
- **Registrable domains** — the **eTLD+1**, e.g. `example.co.uk` — the part you
  could actually register.
- **Both** (default) — both lists, each with a count.

### Why the Public Suffix List matters

Naively splitting on dots gets multi-level suffixes wrong: `example.co.uk` is one
registrable domain, not `co.uk`. This tool validates every candidate against
**Mozilla's Public Suffix List**, so it knows that `.co.uk`, `.gov.uk`, and
thousands of other suffixes are public — and it correctly drops IP addresses,
version numbers like `3.14`, and bogus TLDs.

- **Validated**: only real, registrable domains survive.
- **Deduplicated**: the same domain written twice counts once, in first-seen order.
- **Private**: everything runs **locally in your browser** via WebAssembly —
  your text is never uploaded.

### Handy for

- Building a clean list of every domain mentioned in an email, log, or document.
- Pulling the apex domains out of a pile of URLs for an allowlist or audit.
- Counting how many distinct organisations a blob of links touches.
