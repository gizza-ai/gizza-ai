# base64-validator competitor analysis (2026-08-09)

## Scope

Tool: `base64-validator` — validate a Base64 / Base64url string and explain exactly what is
wrong with it (alphabet, padding, length, canonical trailing bits), rather than just returning
a green/red badge.

Duplicate check first: `blocks/base64url-converter` transcodes between the two alphabets,
`blocks/extract-decode-base64` hunts for Base64 blobs inside prose, `blocks/multi-encoder`
encodes/decodes text (and simply errors on malformed input), and `blocks/format-validator`
covers email/URL/IP/phone/card. None of them reports *why* a Base64 string is invalid, so this
is a distinct diagnostic surface, not a near-dup.

## Sources checked

Web searches: `base64 validator online check if string is valid base64` and
`base64url validator tool padding alphabet errors online`.

Five reachable competitor tools reviewed (paraphrased throughout — no competitor copy,
branding, or trademarks reused):

1. **base64.guru — Base64 Validator** (`base64.guru/tools/validator`). A dropdown of Base64
   *standards* to validate against: the plain standard, MIME (76-character lines), OpenPGP
   ASCII Armor, Base64url, IMAP's modified UTF-7 alphabet, PEM, and two XML Schema token
   flavours. Output is essentially pass/fail; the site pushes users to a separate "standard
   detector" and a separate "repair" tool when a string does not validate. Free, no stated
   limits.
2. **OneDev Tools — Base64 Validator** (`onedev.tools/base64/validator`). Validates as you
   type. Detects three classes of problem: characters outside the alphabet, a wrong number of
   `=` signs, and a length that is not a multiple of four. Advertises error messages that carry
   line/column information, distinguishes standard from URL-safe (`-`/`_`), shows a character
   count, and ships sample data plus a clear button. FAQ covers the allowed character set, why
   validation fails, whitespace, corruption detection, and standard-vs-URL-safe compatibility.
   Free.
3. **Online Base64 Tools — Validate Base64** (`onlinebase64tools.com/validate-base64`). Two
   explicit options: tolerate spaces/tabs inside the data, and make trailing `=` optional.
   Validates character set, four-byte alignment, and that `=` only appears at the end —
   deliberately *without* decoding. Invalid results list the offending characters together
   with their positions. Ships three worked examples (strict text, a data-URI image with
   whitespace, and a malformed string), plus copy/download/import-export and URL parameters for
   automation. Free, client-side only.
4. **B64Encode — Base64 Validator** (`b64encode.com/tools/base64-validator/`). Accepts standard,
   URL-safe, and MIME-style input and validates automatically with no button press. Colour-coded
   pass/fail with no per-problem explanation. Explicitly states the limitation that syntactically
   valid Base64 can still decode to meaningless data. Free, in-browser.
5. **Base64Decode.tools — Base64 Validator** (`base64decode.tools/tools/base64-validator.html`).
   Two modes: a permissive default that tolerates whitespace and the URL-safe alphabet, and a
   strict RFC 4648 §4 mode that rejects both. Reports the specific failure reason (bad character,
   misplaced padding, bad length), auto-detects the variant, and estimates the decoded byte count
   so users can tell whether data was truncated. FAQ covers padding mechanics, whitespace and
   strict decoders, truncation recovery, and regex sanitising. Free, client-side.

## Table-stakes capabilities and fit

| Capability | Competitor pattern | In model? | Decision |
| --- | --- | --- | --- |
| Alphabet check | All five reject characters outside `A–Za–z0–9` + the two symbol slots. | Yes | Per-character scan; every offending character is reported with its 1-based position **and** line/column. |
| Padding rules | All five check `=` count and placement. | Yes | `=` must be trailing only, at most two, and must match the length remainder. |
| Length / 4-alignment | All five. | Yes | Reported as an explicit error, including the "remainder 1 is impossible" case. |
| Standard vs URL-safe | 2, 3, 5 distinguish the alphabets; 1 exposes it as a standard. | Yes | `variant = auto \| standard \| url-safe`; `auto` detects and reports which, and flags a string that mixes `+/` with `-_`. |
| Optional padding toggle | 3 and 5 (permissive vs strict). | Yes | `padding = optional \| required \| forbidden` — `forbidden` matches JWT segments and other unpadded-by-spec uses. |
| Whitespace tolerance toggle | 3 and 5. | Yes | `ignore_whitespace` (default on); when off, each whitespace character is reported as invalid with its position. |
| Line-length standards (MIME 76 / PEM 64) | 1 offers MIME and PEM as standards. | Yes | `max_line_length` (0 = no check, 64 = PEM, 76 = MIME) validates each line of the original text. |
| Decoded byte count | 5 estimates it. | Yes | Exact decoded size, plus whether the payload is text or binary. |
| Data-URI input | 3's second example; a common paste shape. | Yes | A `data:…;base64,` prefix is detected, stripped, and reported as a note. |
| Positions of bad characters | 2 (line/column), 3 (positions). | Yes | Both, capped at 20 reported problems so a pasted binary blob cannot produce a wall of text. |
| Machine-readable output | None of the five. | Yes | `output = text \| json` (family norm here) — a differentiator for scripting/CI. |
| Repair / normalise the string | 1 links to a separate repair tool. | Yes | When every problem is mechanically fixable (whitespace, wrong alphabet, missing padding), a suggested corrected string is offered — in the same tool, not a second one. |
| Canonical trailing bits (RFC 4648 §3.5) | None of the five. | Yes | Non-canonical strings (unused trailing bits set) are reported as a warning: they round-trip differently and strict decoders reject them. |
| Decoded content type | None of the five (this repo's `extract-decode-base64` sniffs magic bytes). | Yes | A small magic-byte sniff (PNG, JPEG, GIF, PDF, ZIP, gzip, …) so "what did it decode to" is answered in one step. |
| OpenPGP ASCII Armor / IMAP-UTF-7 / XSD token standards | 1 only. | Considered, rejected | Armor is a container with a CRC-24 checksum (`blocks/pgp-*` territory) and IMAP's alphabet is not Base64; both would bloat the enum for a rare case. |
| "Repair my truncated file" | 5's FAQ discusses recovery. | Out of model | Truncated payloads cannot be reconstructed; the tool reports truncation-shaped evidence (length remainder, decoded size) instead of pretending to fix it. |
| Server-side batch / API keys / accounts | none of the five gate this | Out of model | gizza tools are browser-local wasm, no account, no server. |

## Descriptor / UX choices

- `input` — the string to check; multiline on the page so pasted MIME/PEM line breaks survive.
- `variant` — `auto` (default) / `standard` / `url-safe`.
- `padding` — `optional` (default) / `required` / `forbidden`.
- `ignore_whitespace` — boolean, default on (matches every permissive competitor default).
- `max_line_length` — integer, default 0 (no check); 64 = PEM, 76 = MIME.
- `output` — `text` (default) / `json`.
- Invalid input is a *result*, not an error: the tool always returns a report. Hard errors are
  reserved for an empty input or an unknown option value, and they name the accepted values.
- Page examples cover a valid padded string, an unpadded URL-safe token, and a broken string
  that exercises the suggested-fix path.

## Worked examples used for verification

- `SGVsbG8sIHdvcmxkIQ==` → valid standard Base64, 13 decoded bytes, text payload.
- `SGVsbG8sIHdvcmxkIQ` with `padding=required` → invalid, "padding is missing".
- `SGVsbG8_d29ybGQ` → URL-safe detected under `auto`; invalid under `variant=standard`.
- `SGVsbG8sIHdvcmxkIQ=?` → invalid character reported at its exact position, plus a suggestion.
- `QUJDRB==` → valid but non-canonical (trailing bits set) — the warning path.

## Limits and edge cases

- Input is capped at 5 MiB of characters; longer input is rejected with a clear message.
- At most 20 individual problems are listed; the count of the remainder is stated.
- The decoded preview shows at most 64 characters of text or 24 bytes of hex.
- Positions are 1-based over the original input (before whitespace removal), so they line up
  with what the user pasted.
