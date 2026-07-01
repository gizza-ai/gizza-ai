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

## FAQ

<details>
<summary>What if my URL already has UTM tags on it?</summary>

Existing `utm_*` parameters are replaced with the values you enter, while
non-UTM query parameters and the `#fragment` stay untouched. Re-tagging the
same link is therefore safe — you never end up with duplicate UTM parameters.

</details>

<details>
<summary>Why do spaces become + instead of %20?</summary>

Values are encoded as `application/x-www-form-urlencoded`, the format Google
Analytics and other platforms expect for query strings: spaces become `+` and
reserved characters are percent-encoded. Both forms decode to the same value.

</details>

<details>
<summary>Which fields are required and which are the GA4 extras?</summary>

Source, medium and campaign are required; term and content are optional. The
four GA4-specific fields — `utm_id`, `utm_source_platform`,
`utm_creative_format` and `utm_marketing_tactic` — are also optional, and any
field left blank is simply omitted from the final URL.

</details>

<details>
<summary>Should I turn on "Lowercase parameter values"?</summary>

Usually yes. UTM values are case-sensitive in analytics reports, so `Email` and
`email` show up as two different sources. The option lowercases every value
before building the link, keeping your reports consolidated.

</details>
