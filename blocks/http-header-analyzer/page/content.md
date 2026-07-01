## About this tool

**HTTP Header Analyzer** takes a block of HTTP **response** headers — the kind
you copy out of `curl -I`, your browser's DevTools Network tab, or a proxy log —
and explains what each one does, then tells you which recommended **security
headers are missing**.

### What it explains

- **Caching** — decodes `Cache-Control` (`max-age`, `no-store`, `public`,
  `immutable`, …), `Expires`, `ETag`, `Last-Modified`, `Age`, and `Vary`, and
  warns when a response has **no caching headers** at all.
- **Compression** — flags `Content-Encoding` (gzip / br / zstd) and
  `Transfer-Encoding`, and notes when a text response is sent **uncompressed**.
- **Content** — `Content-Type` (and warns when a text type has **no charset**),
  `Content-Length`, `Content-Disposition`, `Accept-Ranges`, `Link`, and more.
- **Cookies** — for each `Set-Cookie`, points out missing **`Secure`**,
  **`HttpOnly`**, and **`SameSite`** hardening attributes.
- **CORS** — the `Access-Control-*` family, including the `*` + credentials
  pitfall.
- **Server hints** — `Server`, `X-Powered-By`, `Via`, `X-Cache`, `Alt-Svc`, and
  a fingerprinting reminder.

### Security grade & value quality

It assigns an overall **A+ → F security grade** based on how many of the
recommended security headers are present, and — like a security-headers scanner —
it also grades the **quality of the values** you did send: it flags a CSP that
allows **`unsafe-inline`** or **`unsafe-eval`**, an **HSTS `max-age` too short to
preload**, a **weak Referrer-Policy**, an **obsolete `X-Frame-Options`** value,
the **deprecated `X-XSS-Protection`** header, and information-disclosure headers
(`Server`, `X-Powered-By`, `X-AspNet-Version`) you should trim.

### Missing security headers

It checks for the six commonly-recommended response security headers and lists
each one that is **absent**, with a concrete fix:

- **Strict-Transport-Security** (HSTS)
- **Content-Security-Policy** (CSP)
- **X-Content-Type-Options: nosniff**
- **X-Frame-Options** (clickjacking)
- **Referrer-Policy**
- **Permissions-Policy**

### Example

```
HTTP/2 200
content-type: text/html; charset=utf-8
cache-control: public, max-age=3600
content-encoding: gzip
strict-transport-security: max-age=31536000; includeSubDomains
server: nginx/1.25.0
```

is explained header-by-header, and the analysis flags that **CSP**,
**X-Content-Type-Options**, **X-Frame-Options**, **Referrer-Policy**, and
**Permissions-Policy** are still missing.

### Common uses

- Audit a site's response headers for security gaps before launch.
- Understand why a resource is (or is not) being cached or compressed.
- Decode an unfamiliar header without hunting through the RFCs.

A leading status line (e.g. `HTTP/2 200`) is optional, and both `CRLF` and
bare-`LF` line endings are accepted, so you can paste headers straight from a
log or a terminal.

### FAQ

<details>
<summary>Do I have to strip the "HTTP/2 200" line before pasting?</summary>

No — a leading status line is detected and reported separately, and both CRLF and bare-LF line endings work, so `curl -I` output or a DevTools copy pastes straight in unchanged.

</details>

<details>
<summary>How is the A+–F security grade calculated?</summary>

It starts from how many of the six recommended security headers (HSTS, CSP, X-Content-Type-Options, X-Frame-Options, Referrer-Policy, Permissions-Policy) are present, then deducts for weak values — a CSP with `unsafe-inline`/`unsafe-eval`, an HSTS `max-age` too short to preload, a weak Referrer-Policy, or the deprecated `X-XSS-Protection` header.

</details>

<details>
<summary>Can I analyze request headers with it?</summary>

The explanations target **response** headers — what a server sends back. Request headers will mostly parse but get generic or no commentary, and the security checklist only makes sense for responses.

</details>

<details>
<summary>Does the tool contact the website being analyzed?</summary>

No. It never fetches the URL — you paste headers you already have, and the analysis runs entirely in your browser via WebAssembly. That also means it can't check things only observable live, like certificate details.

</details>
