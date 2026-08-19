# authorization-header-decode — competitor analysis (2026-08-17)

Scan run **before** implementing, per `/create-next-tool` step 4. All competitor notes are
**paraphrased observations of behavior**; no competitor copy, branding, or trademark is reused.

## Duplicate viability check (done first)

The picker offered `authorization-header-decode`. Related existing blocks were inspected before
building:

| Existing block | What it actually does | Overlap verdict |
| --- | --- | --- |
| `basic-auth-header-generator` | `build(username, password, full_header)` → `Basic base64(user:pass)`. Encode direction only; no decode path in `descriptor()` or core. | **Inverse tool, not a duplicate.** Nothing in the repo decodes a Basic value. |
| `jwt-decode` | Full JWT decode: JOSE header + **all payload claims** + signature presence + `exp`/`nbf`/`iat` validation with `leeway`/`now`. | **Adjacent.** Boundary drawn below: we report Bearer *structure* only and never decode payload claims. |
| `jwt-claims-diff` | Diffs claim sets of two JWTs. | No overlap. |
| `http-header-parser` | Splits a raw header block into a case-normalized name→value map (`case`, `duplicates`). Does not interpret any header's value; `Authorization` is not mentioned in its core. | No overlap — it hands you the raw value this tool then decodes. |
| `http-header-analyzer` | Explains **response** headers (caching, compression, CORS, security grade). `Authorization` is a request header and is absent from its core. | No overlap. |
| `aws-sigv4-signer` | Produces an `AWS4-HMAC-SHA256` Authorization header. | Inverse direction; this tool parses that header's credential scope. |

Conclusion: **buildable, not a semantic duplicate.** No block accepts an `Authorization:` header
value and takes it apart.

### The jwt-decode boundary (deliberate, enforced in code)

To avoid becoming a second JWT decoder, the Bearer path reports **structure**, not claims:
segment count and per-segment lengths, base64url validity, JWT-vs-opaque classification, and the
decoded **JOSE header** (`alg`/`typ`/`kid`) — which is what "token structure" means. The payload
claim set is deliberately **not** decoded; the page says so and points at a dedicated JWT decoder
for claim inspection and expiry validation. This keeps the two tools complementary rather than
redundant.

## Competitors reviewed (5)

One candidate (`base64encode.dev/basic-auth-decode`) returned HTTP 403 to the fetcher and was
replaced with `app.webacus.dev`, so five reachable competitors were reviewed as required.

1. **base64.guru — Basic Auth Decode.** Accepts a full header, a bare `Basic …` value, or a naked
   base64 string ("Basic" prefix documented as optional). Adds a batch mode: an "Input Format"
   dropdown (auto-detect, newline/comma/semicolon/tab/pipe separated, JSON, log lines) so many
   credentials decode at once. Output is a single `username:password` text field. Copy + download
   buttons. Long explanatory sections on where Basic auth appears, "base64 is not encryption", and
   alternatives; warns against pasting production credentials into online tools.
2. **Souus Tools — Basic Auth Generator & Decoder.** Tabbed generate/decode UI. Decode strips an
   optional `Basic ` prefix and **splits on the first colon only** (RFC 7617), which its FAQ calls
   out explicitly for passwords containing colons. Copy button; client-side; FAQ covers base64 ≠
   encryption, Unicode/special characters, colon handling, curl usage, privacy.
3. **InBrowser.app — Basic Auth Decoder.** Accepts both `Basic dXNlcjpwYXNz` and a full
   `Authorization: Basic …` line. Outputs **separate** username and password fields, each with its
   own copy button. Ships **pre-filled sample data** (the RFC's `Aladdin` / `open sesame` value)
   plus a Reset button. States first-colon splitting and local-only processing.
4. **PureKit — JWT & Bearer Token Decoder.** Auto-extracts the token from a `Bearer <token>`
   string, then shows header, payload, signature and a computed expiry status. No options. Does
   not describe any handling for **opaque** (non-JWT) bearer tokens — a gap.
5. **Webacus — HTTP.BasicAuth / DECODE.** Minimal single-purpose decoder inside a larger toolbox
   (find/clear/copy/save/undo/redo chrome, history sidebar). Takes `Authorization: Basic …`,
   returns the credential pair. Educational note on base64 being trivially reversible.

Reference consulted for scheme coverage (not a competitor tool): a public HTTP Digest parameter
reference listing `realm`/`nonce`/`qop`/`nc`/`opaque`/`algorithm`, and RFC 7235's generic
`scheme token68 | scheme auth-param-list` grammar.

## Table-stakes → where each landed

| Table-stake (seen at ≥1 competitor) | In/out of model | Where it landed |
| --- | --- | --- |
| Accept full `Authorization: Basic …` line, bare `Basic …`, or naked base64 | in-model | `header` param accepts all three; a naked base64 payload is auto-classified and warned about |
| Strip the scheme prefix case-insensitively | in-model | scheme matched case-insensitively, original spelling reported as `scheme`, canonical as `scheme_canonical` |
| Split credentials on the **first** colon (RFC 7617) | in-model | `basic.username` / `basic.password`; a colon in the password is preserved and noted |
| Separate username and password output fields (not one blob) | in-model | JSON object fields + aligned `text`/`table` rows |
| Copy button on the result | in-model | provided by the shared page runtime (Copy result + Download) |
| Pre-filled worked sample + reset | in-model | four `[[example]]` preset chips + the generator's Reset button; every field has a real placeholder |
| Auto-extract token from `Bearer <token>` | in-model | Bearer path; JWT-shaped tokens classified as `jwt` |
| Show JWT header/alg | in-model | `bearer.jose_header` (alg/typ/kid) — **header only, by design** |
| Show JWT payload claims + expiry status | in-model but **rejected** | duplicate of `jwt-decode`; the page points there instead (see boundary above) |
| "base64 is not encryption" security education | in-model | page copy + a `warnings` entry on every Basic decode |
| Mask credentials so a parse is shareable | in-model | `mask_credentials` (family convention, also mitigates competitors' "don't paste real credentials" warning) |
| Batch decode of many credentials at once (base64.guru) | in-model but **rejected** | one header per run keeps a single stable output object across chat/CLI/page; the CLI is trivially loopable. Recorded, not silently dropped. |
| Machine-readable output | in-model | `format = json` (default) / `text` / `table` |

## Gaps we close that no reviewed competitor covers

- **Every scheme, not just Basic.** `Digest` auth-params are parsed into a real map (quoted-string
  and token values, `realm`/`nonce`/`qop`/`nc`/`cnonce`/`opaque`/`algorithm`/`response`/`uri`),
  `AWS4-HMAC-SHA256` has its `Credential=` scope split into access-key/date/region/service, and
  `Negotiate`/`NTLM` blobs are recognized (NTLM message type read from the `NTLMSSP` signature).
- **Opaque bearer tokens** get a real answer (length, charset, base64url validity, "not a JWT")
  instead of silence.
- **Unknown schemes** still parse: RFC 7235 token68-vs-auth-param detection means a custom
  `Signature`/`Hawk`/`ApiKey` header still yields a structured parameter map.
- **Warnings** for things competitors ignore: a `Basic` payload that is not valid base64 or not
  valid UTF-8, an empty password, whitespace/line wrapping in the pasted value, a missing colon in
  the decoded credentials, and a scheme spelled in a non-canonical case.

## Out-of-model (listed, not built)

- Signature **verification** of any scheme (needs the shared secret/key material and, for sigv4,
  the full canonical request) — a verifier is a different tool.
- Server-side batch upload / log-file ingestion.
- Accounts, history, saved snippets (competitors' toolbox chrome) — no backend here.
