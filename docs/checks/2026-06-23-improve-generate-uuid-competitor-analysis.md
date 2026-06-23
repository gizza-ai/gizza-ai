# generate-uuid — competitor analysis (2026-06-23)

Tool: `blocks/generate-uuid` — generates RFC 4122 / RFC 9562 UUIDs in version 4
(random), 7 (unix-time-ordered), 1 (gregorian-time + random node), and 5/3
(namespace, SHA-1/MD5), plus the `nil`/`max` sentinels, one at a time or in bulk
(up to 1000), with formatting toggles (uppercase, hyphens, braces, urn). Pure
Rust (`getrandom` CSPRNG + `md-5`/`sha1` for namespace versions; a `SystemTime`
clock for v1/v7), so it runs on every backend including the chat Service Worker.
Surfaces: chat + CLI + page. Namespace versions (v5/v3), `nil`, and `max` are
fully deterministic; the page supplies the clock via `js_sys::Date::now()`.

## Competitors surveyed (online UUID/GUID generators)

All notes are **paraphrased** observations of capabilities/UX — no copy,
branding, or assets were reproduced.

1. **uuidgenerator.net** — the category leader. Generates a v1 and a v4 on load,
   a "bulk" page for N v4s, and dedicated pages for v3/v5 (namespace + name) and
   v7. Copy-to-clipboard, plain text/file download. No format toggles beyond
   version choice.
2. **uuidtools.com** — v1/v3/v4/v5 generators, bulk generation, a decode/inspect
   view (extract version/variant/timestamp), and namespace presets (DNS/URL/OID/
   X500) for v3/v5. Also offers an API.
3. **uuid.ramsey.dev / online v7 generators** — v7-focused, emphasize the
   timestamp-ordered, database-friendly property; some show the embedded time.
4. **miniwebtool / browserling / freeformatter GUID generators** — single or bulk
   v4, with formatting options on some: uppercase, hyphen removal, braces, and a
   URN/`urn:uuid:` form. Browserling additionally toggles separators and case.
5. **Online GUID Generator (guidgenerator.com)** — bulk v4 with count, uppercase
   toggle, braces toggle, hyphens toggle, and a "no surrounding text" plain list.

## Capability diff vs our tool

| Capability | Competitors | gizza generate-uuid | Status |
|---|---|---|---|
| v4 (random) | yes (all) | yes (122 CSPRNG bits) | parity |
| v7 (time-ordered, sortable) | yes (newer tools) | yes — 48-bit unix-ms prefix; batch ts increments so it stays lexicographically sortable | parity |
| v1 (time + node) | yes (uuidgenerator/uuidtools) | yes — gregorian ts; node randomized with the multicast bit set (never leaks a MAC) | parity+ |
| v5 (namespace, SHA-1) | yes | yes — DNS/URL/OID/X500 presets or any UUID namespace; RFC vector verified | parity |
| v3 (namespace, MD5) | yes | yes — same namespace handling; known vector verified | parity |
| nil / max sentinels | partial (nil common, max rare) | yes — both | parity+ |
| Bulk generation with count | yes (1..N) | yes (1..1000) | parity |
| Multiple namespace names at once | rare | yes — comma/newline-separated `name` yields one deterministic UUID each | parity+ |
| Uppercase toggle | some | yes | parity |
| Hyphen removal (32-char form) | some | yes | parity |
| Braces (registry GUID form) | some | yes | parity |
| URN (`urn:uuid:`) form | some | yes | parity |
| Namespace presets (DNS/URL/OID/X500) | some | yes (aliases) + raw-UUID namespace | parity |
| Runs locally / nothing uploaded | mixed (many are server-side) | yes — pure Rust, browser-local on the page; CSPRNG client-side | parity+ |

## Gaps considered

- **Decode / inspect an existing UUID** (extract version, variant, embedded
  timestamp) — offered by uuidtools.com. This is a distinct *parse* tool rather
  than a *generator*; the core already exposes `parse_uuid`, but surfacing a
  decoder belongs in a separate `uuid-decode`/`uuid-inspect` tool, not here.
  Listed, not built (out of this tool's scope).
- **Copy-to-clipboard button on the page** — a generic page-chrome feature, not
  tool-specific; the page driver (`site/tool.js`) is shared across all tools and
  out of scope for one block.
- **API endpoint** — gizza already exposes every tool over the chat/LLM API and
  CLI surfaces, so this is covered by the platform.

## Conclusion

Built at full parity-plus on the first pass: it covers every generator version
the surveyed tools offer (v4/v7/v1/v5/v3) plus the `nil`/`max` sentinels, matches
their bulk + formatting (uppercase / no-hyphens / braces / urn) options, adds
multi-name namespace batches and a privacy-preserving randomized v1 node, and
runs entirely locally. The only competitor capability not covered (UUID
*decoding*) is a separate tool, not an improvement to this generator. No
out-of-model features remain.
