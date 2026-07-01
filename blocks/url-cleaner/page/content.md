## URL cleaner

Paste a link and strip the tracking junk — `utm_source`, `utm_medium`,
`utm_campaign`, `fbclid`, `gclid`, `msclkid`, `igshid` and many more — to get a
tidy, shareable URL. Everything runs locally in your browser; the link is never
uploaded.

### What it removes

- **Analytics prefixes:** `utm_*`, `pk_*`, `mtm_*`, `matomo_*`, `ga_*`, `hsa_*`,
  `mc_*`, `oly_*`, `vero_*` and similar campaign families.
- **Click identifiers:** `fbclid`, `gclid`, `gclsrc`, `dclid`, `gbraid`,
  `wbraid`, `msclkid`, `yclid`, `twclid`, `ttclid`, `igshid`, `mc_eid`,
  `mc_cid`, `_hsenc`, `_hsmi`, `mkt_tok` and more.
- **Your own list:** add any extra parameter names (comma-separated) to strip
  alongside the built-ins.

### What it keeps

The scheme, host, path, fragment (`#…`), and every other query parameter — in
their original order and exact encoding. Nothing else is touched, so the link
still works.

### Tips

- Turn on **batch** to clean a whole list of URLs at once, one per line.
- Got a parameter the defaults miss? Add it under *extra params*.

### FAQ

<details>
<summary>Is my link sent anywhere?</summary>

No. The cleaner is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Will it break my URL?</summary>

No — only known tracking parameters (and any you list)
are removed; the path and remaining query stay exactly as they were.

</details>

<details>
<summary>Does it re-encode or reorder anything?</summary>

No. Kept query parameters stay in their original order with their exact
percent-encoding — values are never decoded and re-encoded. The `#fragment`
survives too. If every parameter turns out to be tracking, the dangling `?` is
removed; a URL with no query string at all is returned untouched.

</details>

<details>
<summary>How do I remove a parameter the built-in list doesn't know?</summary>

Type its name into **Extra params to strip** — comma-separated for several,
e.g. `sid,ref,partner`. Names are matched case-insensitively against each
parameter, and your extras are applied on top of the built-in exact names and
prefix families (`utm_*`, `pk_*`, `mtm_*`, …).

</details>

<details>
<summary>Can I clean many links in one go?</summary>

Yes — switch on **Clean each line separately (batch)** and paste one URL per
line. Each non-empty line is cleaned independently and blank lines are kept,
so the output stays line-for-line aligned with your input.

</details>
