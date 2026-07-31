## About this tool

**Bulk Artifact Extractor** scans pasted UTF-8 text for common forensic and incident-response artifacts and reports every hit with:

- `kind` — `email`, `url`, `ipv4`, `domain`, `phone`, `bitcoin`, or `credit_card`
- `value` — the exact matched text
- `offset` — the byte offset where the match starts in your input
- `context` — a short surrounding snippet with whitespace flattened

It is useful for triaging log excerpts, email dumps, copied `strings` output, scraped pages, or text recovered from a disk image. The scan runs locally in your browser; nothing is uploaded.

### Worked example

Input:

```
Contact alice@example.com, see https://data.example.org/path, server 203.0.113.7, call +1 415-555-0132, card 4111 1111 1111 1111.
```

Table output includes rows like:

```
Bulk artifact extractor · 5 artifacts · url 1 · email 1 · credit_card 1 · ipv4 1 · phone 1

| kind | value | offset | context |
| --- | --- | --- | --- |
| email | alice@example.com | 8 | Contact alice@example.com, see https://data.example… |
```

Choose **JSON** when you want to pipe the findings into another script, or keep the default Markdown table for review and reports.

### Options

- **Kinds** — `all` or a comma-list such as `email,ipv4,url`. Domain hits inside emails/URLs and IP hits inside URLs are suppressed before filtering so the output is not double-counted.
- **Output format** — `table` for Markdown or `json` for an array of `{kind,value,offset,context}` objects.
- **Context characters** — how many characters to include on each side of a hit, capped at 200.
- **Maximum findings** — cap results after filtering, in byte-offset order.

### Limits

- The input is treated as UTF-8 text. For binary files, first run a strings-style extraction and paste the text here.
- Phone and domain extraction are heuristic; country-specific numbering plans and private/internal hostnames can need manual review.
- Credit-card numbers must pass the Luhn checksum, but that does not prove a card number is real or active.
- Bitcoin detection recognizes common legacy base58 and bech32-looking addresses; it does not query a blockchain.

## FAQ

<details>
<summary>Does it scan binary files directly?</summary>

No. This tool accepts text in the browser form and chat/CLI schema. For a disk image or binary blob, first extract readable strings with your forensic tool of choice, then paste the UTF-8 text here. The reported offsets are byte offsets in the pasted text, not the original disk image.

</details>

<details>
<summary>Why are domains inside URLs or email addresses not listed separately?</summary>

The extractor resolves overlaps by specificity. `https://example.com/path` is reported as a URL, not as both a URL and `example.com`; `alice@example.com` is reported as an email, not an email plus a domain. That keeps counts useful during triage.

</details>

<details>
<summary>Are credit-card matches validated?</summary>

They must pass the Luhn checksum and contain 13–19 digits, which filters out many random number strings. Luhn validation is only a formatting check: it does not prove the number is issued, active, or sensitive in a legal sense.

</details>

<details>
<summary>Can I limit the scan to only one or two artifact types?</summary>

Yes. Set **Kinds** to a comma-list such as `email,ipv4` or `url,domain`. Use `all` or leave the field blank to report every supported kind.

</details>

<details>
<summary>Is the data uploaded?</summary>

No. The scan is pure WebAssembly running locally in the page. Your pasted text stays in your browser unless you copy or download the output yourself.

</details>
