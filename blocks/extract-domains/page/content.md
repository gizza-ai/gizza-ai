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

### FAQ

<details>
<summary>Why doesn't it extract IP addresses or things like "v1.2.3"?</summary>

Every candidate is checked against the Public Suffix List, and only strings whose suffix is a known ICANN or private entry survive. `192.168.0.1` and `3.14` have no real TLD, so they're dropped on purpose — you get domains, not dotted noise. Use an IP-extraction tool for addresses.

</details>

<details>
<summary>What's the difference between "hostname" and "registrable" mode?</summary>

Hostname mode returns hosts exactly as written (lowercased), e.g. `www.blog.example.co.uk`; registrable mode collapses each to its eTLD+1 — `example.co.uk`, the part you could register. **Both** (the default) returns the two lists side by side with counts.

</details>

<details>
<summary>In what order are the results returned?</summary>

First-seen order by default, which preserves the flow of the source text. Tick **Sort** to get both lists alphabetically instead. Either way each domain appears once — duplicates are removed after lowercasing and trailing-dot stripping, so `Example.COM` and `example.com.` count as one.

</details>

<details>
<summary>Does it find domains inside URLs and email addresses?</summary>

Yes — bare hostnames, hosts inside `http`/`https`/`ftp` URLs, and the part after the `@` in email addresses are all picked up by the same scan.

</details>
