## What this tool does

Generate valid [schema.org](https://schema.org) **JSON-LD** structured data —
the markup Google reads to power rich results — from plain `name = value` field
inputs. Pick a type, list your fields, and copy the JSON-LD straight into your
page's `<head>`. Everything runs locally in your browser: nothing is uploaded, it
works offline, and there's no sign-up.

## How to use it

1. Choose a **Schema type** (Article, Product, FAQPage, Organization, …).
2. In **Fields**, put one `name = value` pair per line.
3. For **FAQPage**, fill in the **FAQ pairs** box instead (`Question? | Answer`).
4. Tick **Wrap in script tag** to get a ready-to-paste `<script>` block.

## Field syntax

| You write | You get |
| --- | --- |
| `headline = My Title` | `"headline": "My Title"` |
| `author.name = Jane Doe` | `"author": { "@type": "Person", "name": "Jane Doe" }` |
| `address.streetAddress = 1 Main St` | nested `PostalAddress` |
| `offers.price = 19.99` | `"offers": { "@type": "Offer", "price": 19.99 }` (a number) |
| `keywords = seo, json-ld, rust` | `"keywords": ["seo", "json-ld", "rust"]` |

- **Dotted names** build nested objects, and the inner `@type` is inferred:
  `author`/`creator` → `Person`; `publisher`/`brand`/`manufacturer` →
  `Organization`; `address` → `PostalAddress`; `offers` → `Offer`;
  `aggregateRating` → `AggregateRating`; `geo` → `GeoCoordinates`.
- **List fields** (`keywords`, `sameAs`, `ingredients`) split on commas into a
  JSON array.
- **Numeric fields** (`price`, `ratingValue`, `reviewCount`, `latitude`,
  `longitude`, `calories`, …) are emitted as JSON numbers, not strings.
- Blank lines and lines starting with `#` are ignored.

## Supported types

`Article`, `Product`, `FAQPage`, `Organization`, `LocalBusiness`, `Person`,
`Event`, `Recipe`, `WebSite`, `BreadcrumbList`, `Review`, `VideoObject`,
`HowTo`, `JobPosting`, `Course`, `SoftwareApplication`. The type name is
case-insensitive, and a blank type defaults to `Article`.

> Note: Google deprecated FAQ rich-result snippets in May 2026, but the
> `FAQPage` markup itself is still valid and helps search engines and AI
> assistants understand your page — so it's kept here.

## Example — a Product with an offer and rating

Fields:

```
name = Acme Wireless Headphones
brand.name = Acme
offers.price = 89.99
offers.priceCurrency = USD
aggregateRating.ratingValue = 4.6
aggregateRating.reviewCount = 214
```

Output:

```json
{
  "@context": "https://schema.org",
  "@type": "Product",
  "name": "Acme Wireless Headphones",
  "brand": { "@type": "Organization", "name": "Acme" },
  "offers": { "@type": "Offer", "price": 89.99, "priceCurrency": "USD" },
  "aggregateRating": { "@type": "AggregateRating", "ratingValue": 4.6, "reviewCount": 214 }
}
```

## FAQ

**Where do I paste the output?** Inside a `<script type="application/ld+json">`
tag in your page's `<head>` (tick *Wrap in script tag* to get the whole block).

**Will this pass Google's Rich Results Test?** The output is valid schema.org
JSON-LD for the chosen type. You still need to supply the properties Google
requires for that type (e.g. a Product usually needs `name` plus `offers` or
`aggregateRating`) — this tool builds the markup, it doesn't invent your data.

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**How do I add a nested object?** Use a dotted field name, e.g.
`author.name = Jane Doe` produces a nested `Person`. Add more dotted lines with
the same prefix to add more properties to that object.
