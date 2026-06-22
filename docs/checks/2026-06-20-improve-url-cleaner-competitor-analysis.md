# url-cleaner — competitor analysis (2026-06-20)

Fourth `/create-next-tool` backlog pick. Pure text tool (Input::None) — full 3
surfaces (chat / CLI / page + query-param deep-link). Research via `WebSearch`.
All findings **paraphrased**.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| ClearURLs (extension) | large community ruleset; per-domain rules; strips utm_*, fbclid, gclid, etc. | capabilities |
| T.LY / Keep / SimpliConvert (online) | one-paste clean; copy clean link; broad default list | capabilities / UX |
| "Remove FBclid and UTM" / "Clean URL" (extensions) | utm_*, ref_*, click ids; one-click | capabilities |
| TextFormatter / utm.io | batch cleaning; explains what was removed | UX |

## Gap diff vs our tool
Our tool: strips a curated set of analytics prefixes (utm_, pk_, mtm_, ga_, hsa_,
mc_, oly_, vero_, __cft, __tn, …) + exact click ids (fbclid, gclid, msclkid,
igshid, yclid, ttclid, ysclid, __hstc/__hssc/__hsfp, …); preserves scheme/host/
path/fragment and remaining params in original order + encoding; batch (per_line)
and user `extra` list.

**In-model gaps closed in this pass:**
- Broader default list — added `ysclid` (Yandex), Facebook `__cft__`/`__tn__`,
  HubSpot `__hstc/__hssc/__hsfp`, `guccounter` from competitor coverage (+ test).

**In-model gaps considered, deferred:**
- **Per-domain rules** (ClearURLs-style) — e.g. strip `si` only on youtu.be /
  open.spotify.com, `ref` only on some hosts. Powerful but needs a domain
  ruleset + matching engine; stripping these globally would break legitimate
  links, so it's a focused follow-up rather than a global default.
- A "what was removed" summary line — minor UX nicety.

**Out-of-model:** live browser-address-bar rewriting (that's an extension, not a
tool), remote rule auto-updates.

## Tested
unit (9: utm+click-id strip, drop-?-when-empty, fragment/order/encoding
preserved, no-query passthrough, extra names, per_line batch, prefix families,
yandex/facebook extras, empty-input error) + drift-guard · wafer fixtures (2) ·
`wafer build` · wasm-pack web · generator · CLI · Playwright page (4 incl.
query-param deep-link).

> Original work only — no competitor copy, branding, or trademarks copied.
