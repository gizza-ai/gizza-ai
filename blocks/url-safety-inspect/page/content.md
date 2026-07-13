## About this tool

The **URL Safety Inspector** reads the *structure* of a link and rates how much it looks
like a phishing URL. It never contacts the site and never queries a blocklist — every
signal comes from the URL string itself, so the check is instant, private, and
deterministic (the same URL always produces the same rating).

It runs these heuristic checks:

- **IP-literal host** — the host is a bare IPv4/IPv6 address instead of a domain name.
- **`@` in the authority** — userinfo before an `@` disguises the real host that follows it
  (a browser connects to the part *after* the `@`).
- **Excessive subdomains** — deep nesting like `login.secure.account.example.com` used to
  bury a lookalike brand.
- **Punycode / homograph labels** — an `xn--…` label that can render as characters mimicking
  a trusted domain.
- **Suspicious or lookalike TLDs** — free/abused TLDs (`.tk`, `.xyz`, `.zip`, …) and
  typo-squats of common ones (`.cm` ≈ `.com`).
- **Percent-encoded hostnames, plain http, URL-shortener hosts, credential/urgency keywords,
  hyphen-stacked hosts, non-standard ports, excessive length, and digit-heavy domains.**

Each match adds to a **0–100 composite score** that maps to a **MINIMAL / LOW / MEDIUM /
HIGH / CRITICAL** rating, and every finding is listed with its severity.

### Worked example

Inspecting `http://paypal.com@192.168.0.1/login` returns:

```
Phishing risk: CRITICAL (score 78/100)
URL: http://paypal.com@192.168.0.1/login

Findings (4):
  [high] Authority contains '@': the browser connects to '192.168.0.1', while the 'paypal.com' shown before the '@' is ignored — a classic disguise.
  [high] Host is an IP literal (192.168.0.1), not a domain name — legitimate sites almost always use a registered domain.
  [low] URL uses plain http:// — traffic is unencrypted and the site presents no TLS identity.
  [low] Contains urgency/credential keywords (login) — common in pages that fake a login or verification prompt.
```

A clean link such as `https://www.example.com/pricing` returns `Phishing risk: MINIMAL
(score 0/100)` with no findings.

### Limits & edge cases

- This is a **structural heuristic rater, not a malware scanner**. A `MINIMAL` rating means
  the URL has no structural red flags — it is **not** proof the site is safe, and a well-built
  phishing page on a clean-looking domain can still score low.
- Conversely, plenty of legitimate URLs trip a signal (a bank's real `login.` subdomain, a
  shortened link, a `.xyz` startup). Read the *findings*, not just the rating.
- It inspects **one URL at a time** and only its structure — it does not follow redirects,
  resolve shorteners, fetch the page, or check certificates or reputation feeds.
- The suspicious-TLD, shortener, and keyword lists are curated snapshots, not exhaustive.

## FAQ

<details>
<summary>Does this tool visit the URL or send it anywhere?</summary>

No. The entire check runs locally in your browser (WebAssembly) using only the text of the
URL. Nothing is uploaded, no request is made to the site, and no blocklist is queried — so
it works offline and never tips off a phishing site that you inspected it.

</details>

<details>
<summary>What does the score and rating actually mean?</summary>

Each finding contributes points by severity (high/medium/low/info), and the total is capped
at 100. The bands are: `0` → **MINIMAL**, `1–19` → **LOW**, `20–44` → **MEDIUM**, `45–69`
→ **HIGH**, `70+` → **CRITICAL**. It is a relative "how suspicious does this *look*" measure,
not a probability that the site is malicious.

</details>

<details>
<summary>Why is a URL I know is safe flagged as risky?</summary>

The heuristics catch patterns that phishing *often* uses, but legitimate sites use some of
them too — real login subdomains, marketing links on `.xyz`, or shortened share links. That's
why every finding is explained: use it to understand *what* looked unusual rather than
treating the rating as a verdict.

</details>

<details>
<summary>What is the `@` (userinfo) trick it warns about?</summary>

In a URL like `http://apple.com@evil.example/`, everything before the `@` is *userinfo* that
the browser ignores for routing — it actually connects to `evil.example`. Attackers put a
trusted brand there so the link *looks* legitimate at a glance. The tool always reports the
real host (the part after the last `@`).

</details>

<details>
<summary>What is punycode and why does it matter?</summary>

Punycode (`xn--…`) encodes non-ASCII characters in a domain. It enables *homograph* attacks:
a label using a Cyrillic `а` or Greek `ο` can look identical to a Latin letter, so `аpple.com`
can visually impersonate `apple.com`. Any `xn--` label is flagged so you can decode and
compare it before trusting the link.

</details>
