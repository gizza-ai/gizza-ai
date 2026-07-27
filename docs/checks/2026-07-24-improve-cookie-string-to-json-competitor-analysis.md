# cookie-string-to-json — competitor analysis (2026-07-24)

Tool function: parse a raw HTTP request `Cookie:` header string
(`name1=value1; name2=value2; …`) into JSON, percent-decoding values.

## Competitors scanned (top 3 reachable; paraphrased, no copy/branding reused)

1. **ToolsForNerds — HTTP Cookie Parser & Decoder** (`toolsfornerds.net/cookie-parser`)
   - Accepts the request-cookie format (`name=value; name=value`) and also a
     `Set-Cookie` string with attributes.
   - Output: pretty-printed JSON.
   - One toggle: "Include decoded values in JSON output" (URL-decode values,
     optional).
   - Two demo/"load example" buttons (Cookie / Set-Cookie).

2. **KeJson — Online Cookie Formatter & Converter** (`kejson.com/en/format/cookie/`)
   - Inputs: native `Cookie:` header string, JSON list
     (`[{"name","value"}]`, the Selenium/Puppeteer/Playwright shape), Python
     requests dict (`{"k":"v"}`), with automatic input-format detection.
   - Outputs: JSON dict, JSON list `[{"name","value"}]`, native `k=v;` string,
     `curl -b`, `document.cookie`.
   - Copy / clear / example buttons.

3. **NexKits — Cookie Converter** (`nexkits.com/en/tools/cookie`)
   - Inputs: Cookie header, JSON (object or array of cookie objects), Netscape
     cookie file; automatic input detection.
   - Processing options: **Decode URL-encoded key/value pairs**, **Convert
     duplicate keys to arrays**.
   - Netscape export defaults (domain/path/Secure/HttpOnly/subdomains).

(`jsonjson.com/cookies-to-json` returned HTTP 403 and was replaced by NexKits.
It advertised three output shapes: name/value object, array of objects with full
properties, and simple key/value pairs.)

## Table-stakes → decision (every item lands in the descriptor or is listed here)

| Capability | Seen at | Fit | Decision |
| --- | --- | --- | --- |
| Parse request `Cookie:` header `name=value; …` → name/value JSON | all 3 | in-model | **Built** — core function. |
| URL-decode (percent-decode) cookie values, as a toggle | ToolsForNerds, NexKits | in-model | **Built** — `decode` boolean, default `true`. |
| Output as a JSON object `{"name":"value"}` | all 3 | in-model | **Built** — `output = "object"` (default). |
| Output as an array of objects `[{"name","value"}]` (Selenium/Puppeteer/Playwright shape) | KeJson, jsonjson | in-model | **Built** — `output = "pairs"`. |
| Duplicate cookie names → collapse to an array (no data loss) | NexKits | in-model | **Built** — object mode collapses repeats into a JSON array; pairs mode keeps every entry in order. |
| Strip a pasted `Cookie:` header-name prefix / surrounding whitespace | (UX, DevTools paste) | in-model | **Built** — leading `Cookie:`/`Set-Cookie:` (case-insensitive) prefix stripped; whitespace trimmed. |
| Strip RFC 6265 DQUOTE-quoted value delimiters (`name="v"` → `v`) | (correctness) | in-model | **Built** — a matched surrounding `"…"` pair is unwrapped. |
| One-click preset examples | ToolsForNerds, KeJson, NexKits | in-model | **Built** — `[[example]]` chips (basic session cookie, encoded value, duplicate names). |
| Copy result / reset | all 3 | in-model | **Built** — provided by the generator's shared page chrome. |
| `Set-Cookie` **attribute** parsing (expires/path/domain/Secure/HttpOnly) | ToolsForNerds | in-model but out of scope | **Considered, rejected** — the request `Cookie:` header carries no attributes; parsing `Set-Cookie` response attributes is a distinct format that deserves its own tool. Documented as a stated limit on the page. |
| Convert TO `curl -b` / `document.cookie` / native `k=v;` string | KeJson | in-model but out of scope | **Considered, rejected** — this tool's single direction is Cookie→JSON; a re-serializer to other cookie encodings is a separate converter. |
| Netscape cookie-file input/output | NexKits | in-model but out of scope | **Considered, rejected** — Netscape-file conversion (with domain/path/expiry defaults) is a different tool; not name/value-header parsing. |
| Auto-detect JSON/dict/Netscape input and reverse-convert | KeJson, NexKits | in-model but out of scope | **Considered, rejected** — same reason: this is a one-direction Cookie-header→JSON parser, not a bidirectional multi-format converter. |

## UX patterns adopted

- Preset example chips (competitors all ship "load example" buttons).
- A `+` in a cookie value is **kept literal** (cookies are not
  form-urlencoded — unlike a query string, `+` is not a space here).
- Errors state what was expected; empty input yields an empty result, not an
  error.
- No competitor branding, copy, or trademarks reused — original copy throughout.
