# vcard-qr — competitor analysis (2026-08-23)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 3.
All notes are paraphrased observations of publicly visible form fields and FAQ points —
no competitor copy, branding or trademarks are reproduced or reused.

## Search

One WebSearch: "vCard QR code generator contact vcf online tool". The result set was dominated by
commercial QR-platform landing pages (myqrcode, qrcodechimp, hovercode, pageloot, qr-code-generator,
the-qrcode-generator, useqrkit, qr-stock, vcardify).

## Competitors skimmed (top 3 reachable)

1. **useqrkit.com/vcard-qr-code** — advertises ~15 contact fields (name, phone, email, company,
   website, address, job title, social links, profile photo). Design controls: module pattern
   presets, eye shapes, logo upload, brand colour, frames, templates. Download: raster free,
   vector behind a paid tier. Every code is *dynamic* (the QR encodes a short link the vendor
   hosts, with scan counting); the free tier caps scans per month and shows an interstitial.
   FAQ points: vCard 3.0 `.vcf` is what gets produced; recommended printed size ≥ ~15 mm square
   with a quiet margin; codes do not expire.
2. **qrcodechimp.com/qr-code-generator-for-vcard** — fields: name, phone number(s), email,
   company, street, city, state, country, website. Design: colour picker, module/eye shape
   presets, logo, background decoration, 3D effect. Download: vector plus fixed raster sizes
   (256 → 4096 px). Optional "make dynamic" checkbox for analytics/editing and a location-tracking
   toggle. FAQ points: static codes work without a network because the data is embedded; dynamic
   codes can be edited after printing.
3. **hovercode.com/tools/vcard-qr-code** — fields: first name, last name, company, job title,
   mobile number, phone number, email, website, street, city, state, post code, country,
   description. No colour/size/error-correction controls on the free static tool. FAQ points:
   iOS and Android both read the vCard payload with the stock camera; a profile photo is refused
   on static codes because it makes the symbol too dense to scan; a static code is permanent, so
   changed details require a new code.

## Table stakes → decisions

| Capability | Competitors | Decision |
|---|---|---|
| First / last name | all 3 | **in-model** — `first_name`, `last_name` (structured `N` + derived `FN`) |
| Company | all 3 | **in-model** — `organization` (`ORG`) |
| Job title | 2 of 3 | **in-model** — `job_title` (`TITLE`) |
| Separate mobile + work phone | 2 of 3 | **in-model** — `mobile` (`TEL;TYPE=CELL`), `phone` (`TEL;TYPE=WORK`) |
| Email | all 3 | **in-model** — `email`, validated (`EMAIL`) |
| Website | all 3 | **in-model** — `website`, bare hosts get `https://` (`URL`) |
| Postal address (street/city/state/postcode/country) | all 3 | **in-model** — 5 params → one `ADR;TYPE=WORK` |
| Free-text note / description | 2 of 3 | **in-model** — `note` (`NOTE`) |
| Birthday | some platform editors | **in-model** — `birthday`, `YYYY-MM-DD` validated (`BDAY`) |
| vCard version choice | implicit (3.0) | **in-model, exposed** — `version` enum `3.0`/`4.0`, correct TYPE casing per version |
| Colour control | 2 of 3 | **in-model** — `foreground` / `background`, hybrid colour control |
| Output size | 2 of 3 | **in-model** — `size` 128–2048 px slider (SVG is vector, so this is the display size) |
| Error-correction level | 1 of 3 (implicit elsewhere) | **in-model** — `error_correction` L/M/Q/H |
| Printed caption under the code | badge/print use case | **in-model** — `show_details` prints the readable contact block under the symbol |
| Vector download | paid on 1, free on 1 | **in-model and free** — the page renders SVG and offers the `.svg` download; the `.vcf` source is embedded in the SVG `<desc>` and returned in the chat/CLI summary |
| Raster (PNG) export | all 3 | **out of model here** — this page's `format = "svg"` path renders vector only; `qr-code-generator` already ships PNG output for the generic case. Listed, not built. |
| Logo overlay, module/eye shape presets, frames, templates | all 3 | **out of model here** — styling belongs to the existing `qr-styled` block, not a second implementation. Listed, not built. |
| Profile photo embedded in the card | 1 of 3 (dynamic only) | **out of model** — a `PHOTO` payload blows past QR capacity; hovercode refuses it too. Documented as a limit on the page. |
| Dynamic QR + scan analytics + edit-after-print | all 3 | **out of model** — requires vendor-hosted redirect + tracking. gizza tools run locally and store nothing; a dynamic code is the opposite of that. Documented as a limit. |
| Social profile links | 1 of 3 | **out of model for now** — needs repeated/`X-SOCIALPROFILE` properties and a multi-value control; `website` covers the single-link case. Listed, not built. |

## UX controls adopted

- `kind = "slider"` for `size` (128–2048, step 32) and `kind = "color"` for both colour fields —
  the same declarative controls `payment-qr` uses, so colour names and `transparent` stay typable.
- `[input.labels]` for friendly `version` and `error_correction` option labels.
- `[[example]]` preset chips (sales-rep card, minimal personal card, print-ready high-ECC card)
  because every competitor ships templates/presets; chips are this repo's declarative answer.

## Differentiators we ship that the scanned competitors do not

- Runs locally: contact details are never uploaded, and no scan tracking exists by construction.
- The exact vCard source is exposed (SVG `<desc>` + chat/CLI summary) instead of being hidden
  behind the image, so it can be pasted into a `.vcf` file or checked field by field.
- Explicit vCard 3.0 vs 4.0 selection with version-correct `TYPE` casing and RFC 6350 §3.2
  line folding / escaping, rather than one fixed dialect.
- Checked inputs: bad email, malformed birthday, out-of-range size, unknown colour and
  over-capacity payloads fail with an actionable message instead of producing an unscannable code.
