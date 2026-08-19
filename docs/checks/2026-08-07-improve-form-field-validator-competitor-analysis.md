# form-field-validator — competitor analysis (2026-08-07)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are
paraphrased summaries of publicly documented feature surfaces — no competitor copy,
branding, or trademarks are reproduced or reused in the tool's page.

Backlog row: `form-field-validator` — *"Validates form values (email, phone, URL, postal
code, credit card) against locale rules and returns per-field errors."* (type hint: pure)

## Duplicate check (done first)

| Existing block | What it does | Why this pick is not it |
| --- | --- | --- |
| `blocks/format-validator` | ONE string against ONE of email/url/ipv4/ipv6/phone/credit-card, or auto-detect | Single value, no field names, **no postal code**, and **no locale**: its phone check is a generic 7–15 digit E.164 length test with no country rules. |
| `blocks/data-validator` | Bulk CSV/JSON **rows** against a `field:rule` DSL (`required`, `type=int|float|bool|date|email|url`, min/max, regex, enum, unique) | Row-oriented bulk linting. Its type vocabulary has no `phone`, no `postal-code` and no `credit-card`, and no country parameter. |
| `blocks/phone-format` | Deep single-number libphonenumber parse/format | One number, one surface; not a multi-field form submission. |
| `blocks/address-parse` | Splits a freeform address into parts (its postcode regexes only *locate* a code while parsing) | Parsing, not per-country validation with an expected-format explanation. |
| `blocks/email-validator`, `blocks/luhn-validate`, `blocks/iban-validator` | Single-purpose single-value checks | Same: no form shape, no locale table. |

Gap that is genuinely new: **a whole form submission validated in one call, per named
field, under a country locale**, including **country-specific postal-code formats**
(no existing block validates a postal code against a country's format) and
**country-aware phone length/prefix rules**. Built.

## Competitors reviewed

1. **CheckTown validator suite** (`check.town`, incl. its postal-code validator page) —
   a large set of one-value-per-page validators: email, phone, URL, postal code, credit
   card, IBAN, BIC, IP, VAT, and more.
2. **Formstack field validation** (`formstack.com/features/field-validation`) — form-builder
   feature that auto-verifies email, phone, credit card, address and date fields inside a form.
3. **Geoapify postcode-formats reference** (`geoapify.com/postcode-formats-around-the-world`)
   + the widely-mirrored international postal-code regex compilations — the de-facto
   reference for per-country postal formats.
4. **Credit-card validator pages** (e.g. `toolsspark.com/credit-card-validator`,
   `testmu.ai` free tools) — Luhn + length + brand detection, some with BIN lookup.

## Table-stakes surface, and where each landed

| Table stake | Seen in | Decision |
| --- | --- | --- |
| Email syntax validation | 1, 2 | **In model** — `email` field type. |
| Phone validation with a country selector | 1, 2 | **In model** — `country` param drives calling code + national-digit-length rules. |
| URL validation (scheme/host/path breakdown) | 1 | **In model** — `url` field type; the error names the missing part. |
| Postal code validated against country format | 1, 3 | **In model** — 36-country format table (the reference tools advertise "40+"). |
| Credit card: Luhn + brand/type detection | 1, 4 | **In model** — `credit-card` field type returns the brand. |
| **Many fields at once, per-field errors** | 2 | **In model** — this is the tool's core shape; `fields` takes `name: value` lines or a JSON object. |
| Required-field enforcement | 2 | **In model** — `required` param (names, or `*`). |
| Type inferred from the field's name | 2 (form builders bind a type per field) | **In model** — inference by name, overridable via `rules`. |
| "Expected format" shown next to the error | 1, 2 ("supporting text with an example") | **In model** — every error states the expected format **and** a valid example. |
| Normalized/canonical value returned | 1 (normalization/standardization) | **In model** — `normalize` param: E.164-style phone, lower-cased email domain, canonically spaced postal code, digits-only card. |
| Masking card numbers in output | payment-form convention | **In model** — `mask_sensitive`, default on. |
| Machine-readable result | 1, 4 | **In model** — `output = text | json`. |
| MX / DNS / SMTP deliverability, disposable-domain lists | 1, and "Form Guard"-style services | **Out of model** — needs network I/O; gizza blocks are offline+deterministic. (A disposable-domain check exists separately as `blocks/disposable-email-detector`.) |
| Carrier / line-type / HLR phone lookup | 1 | **Out of model** — live carrier database. |
| BIN → issuing bank/country lookup | 1, 4 | **Out of model** — licensed BIN database. |
| Address *existence* validation across 240+ countries | 1 | **Out of model** — postal address database. |
| VAT number verified against VIES | 1 | **Out of model** — network lookup. |
| Real-time inline on-blur validation UX | 2, Baymard usability guidance | **Out of model as a behaviour** — gizza pages run one submit at a time; the tool instead reports every field's verdict in a single pass, which is the same information. |

Nothing from the scan was dropped silently: every row above is either a parameter/behaviour
of the shipped tool or is listed as out-of-model here.

## UX controls adopted

- Country `<select>` with friendly labels (`[input.labels]`) — the postal-code competitors'
  primary control.
- `multiline = true` on `fields` and `rules` so a pasted form body keeps its newlines.
- `[[example]]` preset chips (a valid US signup form, a broken one, a UK/GB form, a JSON-body
  form) — the reference tools all prefill a sample value; chips are the gizza equivalent.
- Every error line carries the expected format plus a valid example, mirroring the
  "supporting text shows correct formatting" pattern.

## Deliberate limits (stated on the page)

- Format/locale validation only — never a network lookup, so "well-formed" ≠ "exists" or
  "deliverable".
- Postal formats cover 36 countries; `any` accepts any 2–10 character alphanumeric code.
- Phone checks are calling-code + national-length + trunk-prefix rules, not a full
  carrier-grade number plan.
- Card checks are Luhn + length + brand prefix; no issuer lookup.
