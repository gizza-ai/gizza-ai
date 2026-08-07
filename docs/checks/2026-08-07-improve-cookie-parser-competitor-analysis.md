# cookie-parser — competitor analysis (2026-08-07)

Scan run BEFORE implementation, per `/improve-tool` Phase 2–3. All notes are **paraphrased
observations of feature/UX choices** — no competitor copy, branding, or assets were reused.

## Competitors reviewed

| # | Tool | Angle |
|---|------|-------|
| 1 | OpenReplay — Cookie Parser | Dev-debugging parser, tabbed Cookie vs Set-Cookie input |
| 2 | IO Tools — Set Cookie Parser | Response-side only, JSON output, security-review framing |
| 3 | EverydayTools Pro — Cookie Parser | Widest attribute coverage incl. Priority/Partitioned |
| 4 | DevKitLab — Cookie Parser | Handles both header directions, many Set-Cookie lines at once |
| 5 | EZParser — Cookie Parser | Also accepts a `document.cookie` string; flags-focused output |

## Observed table stakes

- **Both directions**: request `Cookie:` (name/value list separated by `;`) and response
  `Set-Cookie:` (one cookie per line, attributes after the first `;`). Several tools force the
  user to pick a tab; auto-detection is the better UX.
- **Multiple `Set-Cookie` lines** parsed in one paste, with a pasted `Set-Cookie:` header name
  tolerated on each line.
- **Attribute coverage**: Domain, Path, Expires, Max-Age, Secure, HttpOnly, SameSite. The
  broader tools add **Priority** and **Partitioned** (CHIPS); those are cheap to include.
- **Percent-decoding** of names/values with an option to keep raw values.
- **Quoted values** (`name="a b"`) unwrapped per RFC 6265.
- **Per-cookie byte size** shown against the ~4096-byte per-cookie browser limit (OpenReplay).
- **Expires normalization** to an unambiguous date. Countdowns ("expires in 3 days") are common
  but clock-dependent.
- **Output format choice**: table / JSON are both expected; OpenReplay adds text + tree.
- **Validation warnings** for common misconfigurations, most often `SameSite=None` without
  `Secure`, missing `HttpOnly`, and oversized cookies.
- **Local processing** stated prominently (privacy) — every tool claims in-browser parsing.

## FAQ topics competitors answer

- Difference between the `Cookie` request header and the `Set-Cookie` response header.
- `Max-Age` vs `Expires` precedence.
- Why a `SameSite=None` cookie gets rejected.
- Why parsing fails (folded lines, truncated headers, malformed dates).
- Advice to redact real session values before pasting.

## Decisions for our build (in-model)

| Gap | Decision |
|-----|----------|
| Cookie vs Set-Cookie tabs | `mode` = `auto` (default) / `cookie` / `set-cookie` — auto-detect by attribute presence, no tab switching |
| Attribute coverage | Domain, Path, Expires, Max-Age, Secure, HttpOnly, SameSite, Priority, Partitioned + unknown attributes preserved |
| Percent-decoding | `decode` boolean, default on; `+` stays literal (cookies are not form-encoded) |
| Raw attributes | `raw_attributes` boolean, default off — echoes the verbatim attribute segment per cookie |
| Output formats | `format` = `json` (default) / `table` / `csv` / `markdown` |
| Byte size | Always emitted per cookie (`size` = bytes of the `name=value` pair) |
| Expires normalization | Parsed to ISO-8601 UTC (`expires_iso`), deterministic — no clock needed |
| Warnings | `warnings` boolean, default on: `SameSite=None` without `Secure`, missing `HttpOnly`/`Secure`, size over 4096 B, both `Expires` and `Max-Age`, unparseable date, `Domain` with a leading dot |
| Privacy note | Stated on the page — everything runs locally in wasm |

## Considered, not built (out-of-model or rejected)

- **"Load my cookies" from `document.cookie`** (OpenReplay) — out of model: the tool is a pure
  wasm function with no DOM/page access, and the CLI/chat surfaces have no browser.
- **Live expiry countdowns** — rejected: clock-dependent output would be non-deterministic
  across the CLI, chat, and page surfaces and would break the drift/output tests.
- **Cookie serializer / `Set-Cookie` builder** — out of scope for a parser; a separate tool.
- **Tree view rendering** — rejected: the JSON format already conveys nesting, and the page
  renders monospaced text output.
- **Full security grading of a whole header block** — already covered by the existing
  `http-header-analyzer` block; this tool stays per-cookie and structural.

## Relationship to existing blocks (dup check)

- `blocks/cookie-string-to-json` — request `Cookie:` header → name/value JSON only; its own
  description states `Set-Cookie` attributes are **not** extracted. No overlap on attributes.
- `blocks/http-header-parser` — keeps each `Set-Cookie` line as an opaque string.
- `blocks/http-header-analyzer` — grades a header block's security posture; does not decompose
  a cookie into fields.

Confirmed distinct: this tool is the attribute-level `Set-Cookie` decomposer none of them provide.
