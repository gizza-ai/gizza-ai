# js-css-minifier — competitor analysis (2026-07-18)

Pre-build scan for a combined JavaScript + CSS minifier that reports before/after
byte sizes. Sources skimmed (paraphrased only — no competitor copy/branding reused):

- minifier.org — combined JS *and* CSS minifier with a language dropdown.
- 10015.io CSS minifier — shows an estimated size-reduction percentage after minifying.
- Toptal CSS minifier/compressor — minimal single-box CSS minifier + an API.

(Also noted from the search: websiteplanet, freeformatter, devtoollab — all
single-language, browser-local, paste-and-minify with copy/download; nothing
they add beyond the three above.)

## Table-stakes params / UX (tagged in-model / out-of-model)

| capability | competitor | tag | decision |
| --- | --- | --- | --- |
| Language selection JS vs CSS | minifier.org (dropdown) | in-model | `language` enum `auto`/`css`/`js`, default `auto` |
| Strip comments | all | in-model | `remove_comments` boolean, default true |
| Preserve `/*! */` license banners | (js-minify convention) | in-model | kept automatically when stripping comments |
| Before/after size + reduction % | 10015.io | in-model | `report` boolean (default true) prepends a one-line size-report comment |
| Collapse whitespace / drop redundant `;` `:` spaces | all | in-model | core CSS minifier |
| Copy-to-clipboard | all | in-model (platform) | generator gives every text tool a Copy button |
| Download `.min` file | minifier.org, 10015 | in-model (platform) | `format = "text"` pages get a Download link |
| Reset / clear | all | in-model (platform) | generator gives every tool a Reset button |
| Worked example presets | — | in-model (platform) | `[[example]]` chips |
| File upload / drag-drop | minifier.org, 10015 | out-of-model | page input is a paste box; upload is a page-shell nicety, not built |
| Combine multiple files into one | minifier.org | out-of-model | single-input page/skill; multi-file bundling needs a build step |
| Fetch/minify by URL | minifier.org | out-of-model | no network fetch in a pure block; paste the code |
| Value-level rewrites (hex shorten `#ffffff`→`#fff`, strip leading zeros, merge rules) | some CSS minifiers | considered, not built | riskier, meaning-sensitive rewrites; kept to safe structural minification so output is guaranteed equivalent — noted on the page |
| Public HTTP API | Toptal | out-of-model | gizza exposes a CLI + chat skill, not a hosted API |

## Notes on safety / positioning

The distinctive value vs the existing `js-minify` block is (1) CSS support and
(2) the size report. The JS path reuses the proven token-aware `js-minify` core
(strings/regex/template-literals verbatim, ASI-preserving, no identifier
renaming) so behavior is identical to that tool. The new CSS minifier is
deliberately structural-only (whitespace + comments + redundant separators) and
protects strings, `url(...)`, and `calc()`/parenthesized expressions, so the
minified CSS is guaranteed equivalent — no risky value rewriting. Auto-detection
uses a keyword/structure heuristic; the page states you can force the language
if it ever guesses wrong.
