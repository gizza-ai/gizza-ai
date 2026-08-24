# http-header-normalizer — competitor analysis (2026-08-21)

Scan run **before** implementing, per `create-next-tool`. All findings are paraphrased from
public documentation; no competitor copy, branding, or trademarks are reproduced here or on the
tool page.

## Search

One web search for the tool's function ("online HTTP header normalizer / canonicalize header name
casing / sort headers"). Notable: there is no well-known *standalone web* normalizer for a pasted
header block — the established prior art is middleware and gateway configuration. Those are the
real implementations of this function, so they are the competitors skimmed.

## Competitors skimmed (3)

### 1. Middy `http-header-normalizer` middleware (middy.js.org)

- `canonical` (boolean, default off): lowercase every header name by default, canonical
  (`Title-Case`) form when enabled.
- `defaultHeaders` (object): inject a header value when it is missing.
- `normalizeHeaderKey` (function): user-supplied renaming hook.
- Rationale documented: downstream consumers need ONE spelling of each name, because names are
  case-insensitive on the wire but string-compared in code.

### 2. `header-case-normalizer` (npm / GitHub, marten-de-vries)

- Single function: rewrite any casing of a name to the "most common" casing (its example is a
  mixed-case user-agent spelling → `User-Agent`).
- Hyphen-segment title casing, with a table of known names taken from MDN, i.e. an exception list
  is table stakes — plain title casing gets `Etag`, `Www-Authenticate`, `Dnt`, `Te` wrong.
- No options.

### 3. OpenRepose "Header Normalization" filter (openrepose.org)

- **Whitelist**: only the listed header names pass; everything else is removed.
- **Blacklist**: exactly the listed names are discarded; everything else passes.
- Exactly one of the two applies per target section.
- Names are matched case-insensitively.
- Targets can be scoped by URI regex / HTTP method (a gateway concept).
- Value handling is not addressed by the filter at all.

Also noted while scanning (not counted as one of the three): Envoy lowercases HTTP/1.1 header keys
by default and offers a serialization-time casing formatter; `@lambda-middleware/http-header-normalizer`
lowercases names and additionally aliases `referer`↔`referrer`.

## Table stakes → decision

| Capability | Source | In model? | Where it landed |
|---|---|---|---|
| Canonical `Title-Case` name output | all three | yes | `case = canonical` (default) |
| Lowercase name output | Middy, Envoy | yes | `case = lower` |
| Uppercase name output | common in shell/CGI contexts | yes | `case = upper` |
| Leave names as written | Middy (`normalizeHeaderKey` escape hatch) | yes | `case = preserve` |
| Exception table for odd names (`ETag`, `WWW-Authenticate`, `DNT`, `TE`, `Sec-WebSocket-*`, `Content-MD5`, …) | header-case-normalizer | yes | built-in canonical exception map |
| Case-insensitive folding of repeated names | all three | yes | duplicate grouping is case-insensitive |
| Blacklist (drop named headers) | OpenRepose | yes | `drop_headers` (comma list, `x-*` prefix rule) |
| Whitelist (keep only named headers) | OpenRepose | yes | `keep_headers` (same syntax) |
| Value whitespace handling | gap in all three | yes | values are always trimmed; obsolete line folds joined via `unfold` |
| Sorting for diff/compare | the picked row itself | yes | `sort = name` (default) / `none` |
| Duplicate policy | RFC 7230 §3.2.2 practice | yes | `duplicates = combine\|list\|first\|last`, `Set-Cookie` never comma-joined (RFC 6265) |
| Drop valueless headers | OpenRepose-adjacent cleanup | yes | `drop_empty` |
| Copy-pasteable `curl` form | ergonomics gap in all three | yes | `output = curl` |
| Run metrics | none | yes | `output = summary` (CSV) |
| Inject a missing default header (`defaultHeaders`) | Middy | **out of model** | not built — that is request *construction*, covered by `blocks/http-request-builder` |
| Rename headers (`referer`↔`referrer`, arbitrary `normalizeHeaderKey`) | Middy, lambda-middleware | **out of model** | not built — renaming changes semantics; a normalizer must not silently rewrite which header you sent |
| URI-regex / HTTP-method scoping | OpenRepose | **out of model** | gateway routing concept; there is no request pipeline here, only pasted text |
| Structured JSON map output | Envoy/config tooling | **deliberately excluded** | `blocks/http-header-parser` already owns headers → JSON (case + duplicate policy included). This tool stays text-in/text-out so the two do not overlap |

## Duplicate check

Nearest existing blocks, all confirmed distinct:

- `blocks/http-header-parser` — headers → **JSON map** (`case`, `duplicates`); structured output,
  first-seen order, no sorting, no allow/deny lists, no curl form.
- `blocks/http-header-analyzer` — security/quality **analysis** of a header block.
- `blocks/http-headers-diff` — compares **two** header blocks.
- `blocks/authorization-header-decode`, `blocks/basic-auth-header-generator` — single-header
  credential tools.

This tool is the text→text formatter of the family: a header block in, a canonical header block
(or `curl` flags) out, sorted and deduplicated, so two captures of the same request diff cleanly.

## UX patterns adopted

- Friendly `<select>` labels via `[input.labels]` for every enum, with the default marked.
- `[[example]]` preset chips: canonical default, lowercase HTTP/2 style, curl flags, allowlist
  cache-key style, duplicate `Set-Cookie` handling.
- `multiline = true` on the pasted header block so newlines survive.
- Worked before/after example plus stated limits in the page copy; five `<details>` FAQs.
