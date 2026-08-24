# url-trailing-slash-normalizer competitor analysis — 2026-08-20

## Scope

New tool: `url-trailing-slash-normalizer`, a browser-local batch URL utility for adding or removing trailing slashes while preserving query strings, fragments, file-like paths, and input order.

## Competitor scan (paraphrased)

Search query: bulk URL trailing slash add remove normalizer online SEO tool trailing slash canonical URL.

| Competitor | Table-stakes observed | Fit for this tool |
| --- | --- | --- |
| URLToolKit URL Normalizer | SEO-oriented canonicalization, lowercasing hosts, default-port removal, query sorting, deduplication, and bulk comparison workflows. | Partly in model. This block intentionally focuses on trailing slashes but includes dedupe and report/summary outputs for audit workflows. Host casing/query sorting are out of scope so the tool can promise only the slash changes. |
| SHRTX URL Normalizer (SEO) | Browser-local URL normalizer with trailing slash removal, parameter sorting, and SEO copy around duplicate canonical URLs. | In model for trailing-slash normalization and privacy copy. Parameter sorting is deliberately not built because preserving query bytes is safer for redirect lists. |
| OneDev Tools URL Normalizer | Developer utility for standardizing URLs for comparison, including lowercase hostnames, sorted query parameters, and default cleanup. | Partly in model. We include line-by-line batch handling, invalid-line policy, and machine-readable outputs; broader URL canonicalization is listed as out-of-model for this focused tool. |

## Parameter and UX decisions

| Need | Decision | Rationale |
| --- | --- | --- |
| Batch input | `urls` multiline text area, one URL per line | Matches sitemap/crawl-export/link-audit workflows and preserves ordering. |
| Direction | `mode` enum: `add` or `remove` | The core question for a trailing slash tool; values are explicit for CLI and query params. |
| File-like paths | `skip_file_paths` checkbox, default true | Prevents dangerous rewrites such as `/sitemap.xml/` or `/style.css/`. |
| Root behavior | `normalize_root` checkbox, default true | Site roots should be rendered as `/`; making this explicit avoids surprising remove-mode output. |
| Duplicates | `dedupe` checkbox, default false | Competitors highlight URL comparison/deduplication; keeping it optional preserves input rows by default. |
| Invalid lines | `on_invalid` enum: `keep`, `drop`, `error` | Real lists contain comments, notes, mailto links, and malformed rows. |
| Outputs | `urls`, `changed`, `report`, `summary` enum | Supports copy-paste normalized lists, redirect lists, audit CSVs, and counts. |
| Examples | Preset chips for add, remove, changed-only, report, and dedupe summary | Gives the generated page task-specific affordances without custom JavaScript. |

## Out-of-model or deliberately not built

- Fetching URLs to discover server redirects/status codes: requires network checks and is not a pure browser-local text transform.
- Lowercasing hosts, sorting query parameters, removing default ports, or decoding/encoding paths: those are broader canonicalization operations. This tool is intentionally byte-preserving except for trailing slash placement.
- Sitemap crawling or recursive link extraction: separate tools already cover URL extraction and sitemap-oriented workflows.

## Verification intent

The page spec asserts exact normalized text, a query-param deep link, enum choices, non-default checkbox states, report output, and the generated CLI example. CLI verification uses an exact-output add-mode case plus a summary/dedupe case.
