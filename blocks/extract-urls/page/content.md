## About this tool

**Extract URLs** scans a block of text and pulls out every **http/https** URL it
contains — validated, deduplicated, and in first-seen order. Tick **Split into
components** to also break each URL into its **scheme**, **host**, **port**,
**path**, **query**, and **fragment**.

- **Validated**: candidates are parsed by a real URL parser, so malformed
  fragments are dropped.
- **Clean**: trailing prose punctuation (the period at the end of a sentence) is
  trimmed, and URLs wrapped in `( )` or `[ ]` come out without the brackets.
- **Deduplicated**: the same URL written twice counts once.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Collecting every link out of an email, document, or chat log.
- Auditing the query parameters / hosts referenced in a blob of text.
- Building a clean, unique link list from messy input.

## FAQ

<details>
<summary>Does it pick up links without http://, like www.example.com?</summary>

No. Only `http://` and `https://` URLs are matched — a bare `www.example.com`
or an `ftp://` address is ignored. This keeps the results high-precision: every
candidate is also run through a real URL parser, so malformed matches are
dropped rather than guessed at.

</details>

<details>
<summary>What happens to punctuation stuck to the end of a URL?</summary>

Trailing prose punctuation — `.`, `,`, `;`, `:`, `!`, `?` — is trimmed, so
"visit https://example.com/page." extracts `https://example.com/page`. URLs
wrapped in parentheses or square brackets come out without the brackets.

</details>

<details>
<summary>If the same link appears several times, do I get it more than once?</summary>

No — the list is deduplicated. Each unique URL appears once, in the order it
was first seen in the text, and the tool reports the total count alongside the
list.

</details>

<details>
<summary>What exactly does "Split into components" break out?</summary>

For every extracted URL it lists the scheme, host, port, path, query, and
fragment separately — handy when you want to audit which hosts are referenced
or compare query parameters across links.

</details>
