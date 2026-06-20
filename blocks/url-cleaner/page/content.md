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

**Is my link sent anywhere?** No. The cleaner is compiled to WebAssembly and runs
entirely in your browser tab.

**Will it break my URL?** No — only known tracking parameters (and any you list)
are removed; the path and remaining query stay exactly as they were.
