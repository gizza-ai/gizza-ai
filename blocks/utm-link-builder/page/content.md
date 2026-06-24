## About this tool

The UTM Link Builder adds campaign-tracking parameters to any URL so your web
analytics can attribute visits to the right marketing source. Fill in the
destination URL and the campaign details, and the tool appends the standard UTM
query parameters, correctly URL-encoded and ready to share.

### What are UTM parameters?

UTM parameters are tags added to a link's query string that analytics tools
(Google Analytics and most others) read to report where traffic came from. The
five standard ones are:

- **utm_source** — the referrer, e.g. `google`, `newsletter`, `twitter`.
- **utm_medium** — the marketing medium, e.g. `cpc`, `email`, `social`.
- **utm_campaign** — the campaign name, e.g. `spring_sale`.
- **utm_term** *(optional)* — a paid-search keyword.
- **utm_content** *(optional)* — distinguishes ads or links in the same campaign.

Source, medium and campaign are required; term and content are optional.

For Google Analytics 4 there are four more optional fields, all supported here:

- **utm_id** — the Campaign ID used to join ads/cost data to a campaign.
- **utm_source_platform** — the platform that directed the traffic (e.g. `Google Ads`).
- **utm_creative_format** — the creative type (e.g. `display`, `video`).
- **utm_marketing_tactic** — the targeting criteria (e.g. `remarketing`).

### How it works

- Values are encoded as `application/x-www-form-urlencoded` — spaces become `+`
  and reserved characters are percent-encoded, exactly as analytics platforms
  expect.
- If your URL has no scheme, `https://` is assumed.
- Existing non-UTM query parameters and the URL fragment (`#section`) are kept.
- Any `utm_*` parameters already on the URL are replaced, so re-tagging a link
  is safe and idempotent.
- Tick **Lowercase parameter values** to normalise casing so `Email` and `email`
  don't show up as two different sources in your reports.

Everything runs locally in your browser — your URLs are never uploaded.
