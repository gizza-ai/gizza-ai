# http-request-builder — competitor analysis & differentiation

**Tool:** `gizza-ai/http-request-builder` — build a well-formed raw HTTP/1.1
request message from a method, URL, headers, and body (nothing is sent).
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| Hand-writing the request | DIY | Easy to get the request-target, `Host`, CRLF line endings, or `Content-Length` wrong. |
| Postman / Insomnia "code" export | App | Heavyweight apps; export raw HTTP is buried, and they're built around *sending*. |
| `curl -v` (read the `> ` lines) | CLI | Shows the request, but you must send it (and parse verbose output) to see it. |
| Online "raw HTTP request" generators | Web | Rare; some upload data; correctness varies. |

## How gizza's tool is better / different

1. **Offline by design — nothing is sent.** It produces the exact wire bytes, so
   you can paste them into `nc` / `openssl s_client`, a teaching slide, or a test
   fixture. (To actually send, use the sibling **http-request** tool.)
2. **Correct framing for free.** Derives the **request-target** (path+query) and
   the **`Host`** header from the URL, adds **`Content-Length`** for a body, and
   uses **CRLF** line endings — the details people get wrong by hand.
3. **Respects your overrides.** If you supply your own `Host` or `Content-Length`,
   it won't duplicate them.
4. **Local + three surfaces.** Chat, CLI, and a zero-upload page, one Rust core
   (URL parsed by the `url` crate).

## Verification

Six core unit tests cover a simple GET, empty-path → `/`, POST body →
auto `Content-Length`, non-default port in `Host`, user `Host`/`Content-Length`
overrides (no duplication), and error cases. **End-to-end CLI** produced a
correct POST message (request-target with query, auto `Host`, headers, auto
`Content-Length: 17`, blank line, body). Page Playwright covers POST + default
GET.

## Scope / honest limitations

- HTTP/1.1 only (the ubiquitous text framing); not HTTP/2/3 (binary).
- It builds the message; it doesn't validate header *semantics* beyond requiring
  `Name: Value` syntax.

## Possible future enhancements

- Optional `Connection: close` / `User-Agent` defaults toggle.
- Generate the equivalent `curl` command line.
- Chunked-transfer body framing.
