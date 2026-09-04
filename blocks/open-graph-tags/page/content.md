## About this tool

Open Graph and Twitter Card tags control the link preview shown when a page is shared in social feeds, chat apps, and team tools. This generator turns a title, description, canonical URL, preview image, and social handles into a copy-pasteable `<head>` snippet with standard meta tags, `og:*` properties, and optional `twitter:*` and `itemprop` tags.

Worked example:

- Title: `How to bake sourdough`
- Description: `A step-by-step sourdough guide covering starter, autolyse, bulk ferment and bake.`
- URL: `https://example.com/sourdough`
- Image: `https://example.com/og/sourdough.png`
- Output includes `<meta property="og:title" content="How to bake sourdough">` and `<meta name="twitter:card" content="summary_large_image">`.

Use the preset chips for common page shapes such as an article, product page, profile, or Open-Graph-only snippet. Leave optional fields blank to omit their tags. The checks comment at the bottom flags advisory issues like relative URLs, missing images, or title/description lengths likely to be truncated.

## Limits and edge cases

- The tool generates markup; it does not fetch your live page, upload images, or call platform validators.
- Social crawlers expect absolute `http://` or `https://` URLs for the page URL and image URL. Relative paths are emitted if you ask for them, but the checks block warns about them.
- Image dimensions are capped at 10,000 pixels per side. Use `0` or blank to omit `og:image:width` and `og:image:height`.
- Values are HTML-escaped for safe attribute output, including ampersands, angle brackets, quotes, apostrophes, and newlines.
- Preview appearance differs by platform and cache state. Re-scrape in each platform's own debugger after publishing if you need to refresh a cached card.

## FAQ

<details>
<summary>Which fields are required?</summary>

Only the page title is required. A useful rich preview usually also needs a description, absolute canonical URL, image URL, image alt text, and site name. The tool omits tags for blank optional fields and explains missing pieces in the checks comment when warnings are enabled.

</details>

<details>
<summary>What size should the preview image be?</summary>

A 1200×630 image works well for the wide 1.91:1 card shape used by many platforms. Square thumbnails can work with `twitter_card=summary`, but Open Graph previews commonly crop toward the wide shape. Set both width and height when you know them so crawlers can reserve layout space.

</details>

<details>
<summary>Should I include both Open Graph and Twitter Card tags?</summary>

Usually yes. Open Graph covers Facebook, LinkedIn, Slack, Discord, and many other unfurlers. Twitter Card tags let X choose the intended card layout and image. If you want only Open Graph, turn off `include_twitter`.

</details>

<details>
<summary>Why are there warnings inside an HTML comment?</summary>

Warnings are advisory, not fatal. Keeping them in a trailing `<!-- Checks -->` comment makes the generated snippet self-documenting while staying harmless if you paste it into a page. Turn off `warnings` when you want only the tags.

</details>
