## About this tool

Phishing emails often make a link look safe while sending you somewhere else: the visible text says
`paypal.com`, the `href` points at an IP address, a shortener hides the target, or a domain swaps a
letter for a digit. This tool scans the links in a pasted email and gives you a copy-pasteable
report for every URL it finds.

Paste a raw email with headers, an HTML body, or plain text. The scanner extracts both `<a href>`
links and bare `http://` / `https://` URLs, compares visible link text with the real target, checks
hosts against built-in and custom brand domains, unwraps one level of common `?url=` redirect
wrappers, and flags structural red flags such as punycode, bare IP hosts, `@` userinfo, shorteners,
plain HTTP, suspicious TLDs, deep subdomains and credential words in the URL.

It is intentionally offline and deterministic. It does not fetch links, follow live redirects, query
WHOIS, check DNS, call a threat feed, inspect SSL certificates, or run an ML classifier. A MINIMAL
rating means the link did not match these structural rules — not that the destination is proven safe.

### Worked example

Paste this email:

```
From: "PayPal Security" <alerts@paypa1-secure.com>
Subject: Urgent: verify your account

<p><a href="http://192.0.2.9/login">https://www.paypal.com/signin</a></p>
<p><a href="https://www.paypal.com/help">Help centre</a></p>
```

The first link is flagged because the visible text says `paypal.com` while the target is an HTTP
link to a bare IP address. The sender domain is also treated as a protected brand candidate, so
lookalike domains around it are caught in the same run. The genuine `paypal.com/help` link remains
listed with a low or minimal rating so you can see the full message context.

### Inputs and report styles

- **Email message** accepts raw RFC 5322, HTML or plain text up to 1 MiB.
- **Your own domains to protect** adds comma-, space- or newline-separated brand domains to the
  built-in list. Use this for your company, product, bank, or customer domains.
- **Read the input as** can auto-detect, or force raw email, HTML body, or text.
- **Report style** switches between detailed text, a compact summary, and JSON.
- **Show only flagged links** hides clean links while keeping totals based on the whole email.
- **Maximum links to scan** defaults to 200 and accepts 1–1000. Extra links are counted as
  truncated rather than silently ignored.

### Limits and edge cases

- The brand list is curated, not exhaustive. Add domains you care about in the custom brand field.
- Lookalike matching is intentionally heuristic: it catches common digit swaps, homoglyph folding,
  edit-distance typos, combosquats and wrong-TLD shapes, but it can produce false positives on short
  or generic domains.
- Redirect wrappers are unwrapped only when the destination appears in the URL query string. The
  tool does not make network requests or expand shorteners.
- SafeLinks, URLDefense and similar services have many variants; this tool flags them as wrappers
  and checks obvious embedded `url=` destinations, while the dedicated decoder block remains the
  better choice for provider-specific decoding.
- JavaScript, data and file schemes are treated as dangerous when they appear in an anchor href, but
  bare-text extraction only scans HTTP and HTTPS URLs.

## FAQ

<details>
<summary>Can this tell me whether a link is definitely safe?</summary>

No. It is an offline structural scanner, not a live reputation service. It can explain suspicious
patterns such as a display-target mismatch, a lookalike domain, a shortener or a bare IP host. It
cannot know whether a clean-looking domain was compromised today or whether a page serves malicious
content after login.

</details>

<details>
<summary>Does it click or fetch any links?</summary>

No. The scanner runs locally on the text you paste and never opens, fetches, expands or resolves any
URL. That keeps it safe for incident triage and reproducible in the browser, CLI and chat surfaces.

</details>

<details>
<summary>How do I scan links that impersonate my own company?</summary>

Add your domains to **Your own domains to protect**, for example `example.com, example.co.uk`. The
scanner compares every link host against those domains plus the built-in brand list and the email's
sender domain, then flags close lookalikes and brand names embedded in suspicious hostnames.

</details>

<details>
<summary>Why is an Outlook SafeLinks or Proofpoint URL flagged?</summary>

Those services wrap the real destination inside another URL, so the visible host is not the final
host a user may reach. This tool flags that as a redirect-wrapper signal and scans an obvious
embedded `url=` destination when present. It does not perform provider-specific decoding or follow
live redirect chains.

</details>

<details>
<summary>What should I do with a HIGH or CRITICAL result?</summary>

Treat it as a triage signal: preserve the email, do not visit the links from a normal browser, and
send the report to your security or IT workflow. The finding list is designed to be pasted into a
ticket so another analyst can see exactly which link triggered which rule.

</details>
