# email-obfuscator — competitor analysis (2026-06-23)

Tool: `blocks/email-obfuscator` — encode an email address into scraper-resistant
HTML (numeric entities, JS char-code writer, CSS bidi reversal, ROT13 mailto) so
address-harvesting bots can't read it while the address stays clickable in a
browser. Pure-compute, runs locally on all surfaces (chat / CLI / page).

## Surfaces verified

- **Chat block:** `wafer build` validates + instantiates the wasm32-wasip1 block
  (308.5 KiB). Schema drift-guard unit test passes.
- **CLI:** `gizza tool email-obfuscator …` for entities (decimal/hex), js, css,
  rot13, link on/off, custom link_text, and a rejected invalid address.
- **Page:** Playwright spec `tool-page-email-obfuscator.spec.ts` (4 cases incl.
  hex-no-link checkbox path + a `?email=…&mode=…` deep-link). All pass.
- **Unit tests:** 13 core/block tests (every mode, validation errors, ROT13
  round-trip).

## Top competitors surveyed

1. **Email Address Obfuscator / "Hide my email" generators** (e.g. the classic
   `dynamicdrive.com` Email Riddler, `automaticlabs`-style entity encoders).
   Output: HTML decimal-entity string, optional mailto anchor.
2. **mailtoencoder.com / "obfuscate email" web tools.** Entity + JavaScript
   `document.write` char-code output.
3. **WordPress anti-spam plugins (e.g. the ROT13 mailto trick used by core
   `antispambot()` + many plugins).** ROT13-scrambled href decoded on click.
4. **CSS reversal demos (CSS-Tricks "spam-proof email").** `direction:rtl;
   unicode-bidi:bidi-override` reversed text.
5. **Cloudflare email obfuscation (Scrape Shield).** Runtime JS decode of an
   entity-hex token. (Server feature, not a paste-in snippet — out of model.)

## Capability matrix (after improvements)

| Capability                                   | Competitors | This tool |
|----------------------------------------------|-------------|-----------|
| HTML numeric entities (decimal)              | most        | ✅ default |
| Hex entity radix (`&#x6a;`)                   | some        | ✅ `entity_style=hex` |
| Optional clickable `mailto:` anchor          | most        | ✅ `link` |
| Custom visible link text                     | some        | ✅ `link_text` |
| JavaScript `document.write` char-code build  | some        | ✅ `mode=js` (+ `<noscript>` fallback) |
| CSS bidi-override reversal                    | few         | ✅ `mode=css` |
| ROT13 mailto decoded on click                | WP/plugins  | ✅ `mode=rot13` |
| `<noscript>` graceful degradation            | rare        | ✅ js + rot13 modes |
| Address validation before encoding           | rare        | ✅ local@dotted-domain |
| 100% local / no server / no sign-up          | varies      | ✅ pure wasm |
| API/CLI access (not just a web form)         | rare        | ✅ chat skill + `gizza` CLI |

## Gaps closed in this build

- Added all four obfuscation strategies competitors split across separate tools
  (entities, JS, CSS, ROT13) into a single tool with a `mode` selector — matches
  or exceeds the breadth of any single competitor.
- Added hex entity radix (some tools are decimal-only).
- Added a `<noscript>` entity fallback to the JS and ROT13 modes (most JS-only
  competitors silently fail with scripting disabled — the address vanishes).
- Added pre-encode validation (`local@domain` with a dotted domain) so typos are
  reported instead of silently producing a broken snippet.
- Added optional custom link text and a link on/off toggle.

## Out-of-model (not built — by design)

- **Server-side runtime obfuscation** (Cloudflare-style token injected at the
  edge + decoded by a served script): gizza tools are paste-in snippets / local
  compute, not an edge proxy. Not applicable.
- **Image-rendered email** (rasterize the address to a PNG): possible via the
  `image` crate but a distinct media tool, not part of an HTML-snippet generator;
  deferred to a separate tool if requested.
- **Per-character random scheme mixing** (mix entities/JS per char): marginal
  anti-scrape gain over the deterministic per-mode output; not worth the
  un-reviewable, non-deterministic output.

## Copy / branding

No competitor copy, branding, or trademarks were reused. All titles, tags, and
SEO copy in `page/meta.toml` + `page/content.md` are original and describe the
generic technique.
