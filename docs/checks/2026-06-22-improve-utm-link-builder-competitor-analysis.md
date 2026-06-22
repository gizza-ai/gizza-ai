# utm-link-builder — competitor analysis (2026-06-22)

## Tool
`utm-link-builder` — appends UTM campaign parameters to a URL for analytics
tracking. Pure-Rust block (chat + CLI + page), no network, no model.

## Surfaces verified (Phase 1)
- **Chat block.wasm** — built + validated by `wafer build` (instantiates clean, 314.7 KiB).
- **CLI** — `gizza tool utm-link-builder url=… source=… medium=… campaign=…` returns the
  tagged URL + query + params JSON; missing-required errors with exit 2.
- **Page** — `/tools/utm-link-builder/`; 5 Playwright tests pass (basic build, optional
  term/content encoding + existing-query/fragment preservation, lowercase checkbox, GA4
  utm_id/platform fields, and query-param deep-link prefill).
- Drift-guard schema test passes (10 core unit tests + 1 schema test).

## Competitors reviewed (top 5)
1. **Google GA4 Campaign URL Builder** (ga-dev-tools.google) — the reference implementation.
2. **utmbuilder.com** — dropdowns, saved templates, naming-convention enforcement, link
   shortener, branded QR codes, team workspaces.
3. **CampaignTrackly** — campaign data governance, UTM governance, branded short links, QR
   automation, GA4/Adobe analytics readiness.
4. **Uplifter** — forced consistent format via dropdowns/fixed text/macros/data pickers.
5. **Analytify / utmbuilder.net** — WordPress + GA-friendly fill-in-the-box builders.

## Capability diff + gap ranking (fit-to-model)

| Capability | Competitors | Before | Action |
|---|---|---|---|
| utm_source / medium / campaign (required) | all | yes | kept |
| utm_term / utm_content (optional) | all | yes | kept |
| **utm_id (GA4 Campaign ID)** | Google, most | **no** | **ADDED** |
| **utm_source_platform (GA4)** | Google | **no** | **ADDED** |
| **utm_creative_format (GA4)** | Google | **no** | **ADDED** |
| **utm_marketing_tactic (GA4)** | Google | **no** | **ADDED** |
| Correct value URL-encoding (space→`+`) | all | yes | kept |
| Preserve existing query params + fragment | best builders | yes | kept |
| Idempotent re-tagging (replace existing utm_*) | few | yes | kept |
| Lowercase value normalization (GA best practice) | utmbuilder.com, Uplifter | yes | kept |

### Gaps closed this run
Google documents **nine** campaign parameters; the tool shipped with five. Added the four
GA4 fields — `utm_id`, `utm_source_platform`, `utm_creative_format`, `utm_marketing_tactic`
— as optional string params across core, descriptor (chat schema), CLI, web wrapper and the
page, with `utm_id` ordered first per GA convention. This brings the tool to full parity with
the official Google standard, which most competitors only partially cover.

### Out-of-model features (NOT built — documented only)
- **Link shortening / branded short links** (utmbuilder.com, CampaignTrackly) — requires a
  hosted redirect service + persistent storage; out of scope for a stateless pure block.
- **QR code generation from the tagged URL** — a separate concern; gizza already has a
  `qr-generate`-style block, so chaining is the right pattern rather than duplicating here.
- **Saved templates / presets / team workspaces / naming-convention governance**
  (utmbuilder.com, CampaignTrackly, Uplifter) — all require server-side persistence and
  accounts; the gizza tool model is stateless and client-side, so these are not buildable.
- **Bulk/CSV generation** (Uplifter) — a multi-row batch UI; the single-URL page model and
  the chat/CLI single-call shape don't fit a spreadsheet input. Deferred.

## Note on trademarks/copy
No competitor copy, branding, or trademarks were copied. Parameter names (`utm_*`) are the
public Google/Urchin standard, not proprietary.
