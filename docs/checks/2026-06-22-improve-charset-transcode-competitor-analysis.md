# charset-transcode — competitor analysis (2026-06-22)

New tool: **Mojibake Fixer / Charset Transcoder** — re-decode garbled text
("mojibake") from a legacy charset into clean UTF-8. Built end-to-end this run,
then improved against the competitor landscape below. All copy/design here is
**original**; competitors were analyzed for ideas/features/UX only — no copy,
branding, or trademarks were reproduced.

## What the tool does

The input string's characters are re-encoded to bytes under a chosen legacy
charset, then those bytes are decoded as UTF-8 — the standard inverse of the
"UTF-8 bytes wrongly shown through Windows-1252" corruption that produces
`Ã©`/`â€œ` style mojibake. Surfaces: chat block (LLM API), `gizza` CLI, and the
`/tools/charset-transcode/` page (with query-param deep-links).

## Competitors reviewed (paraphrased)

1. **charset.org — Fix Garbled Text.** Client-side. Targets double-encoded UTF-8
   and Windows-1252/Latin-1 mojibake. Multiple "fix strategy" buttons (no
   auto-detect — user tries buttons). Has an HTML-entity fix button. Suggests
   "two passes" for stubborn cases. Copy/preview UX.
2. **fixgarbled.com.** Free, auto-detect + repair, emphasises CJK (UTF-8, GBK,
   Big5, Shift-JIS). Paste-in, auto-fix.
3. **PunkFix — Mojibake Decoder.** "Fix `Ã©` to `é`." Auto-detect + repair of
   Windows-1252/UTF-8 mismatches.
4. **mytexttool.com — Mojibake Decoder.** Decode UTF-8 misread as Latin-1 /
   Windows-1252. Single instant fix.
5. **TextTools.cc — Broken Encoding Fixer.** Describes the exact "read as Latin-1
   bytes, decode as UTF-8" mechanism; auto-attempts common corrections and shows
   results for verification.

(Five real competitors found; representative of the category.)

## Gap diff vs our v1 (text + explicit `from` + `errors`) and what we did

| Gap (from competitors) | In/out of model | Action |
| --- | --- | --- |
| **Auto-detect the charset** (the dominant differentiator — most rivals auto-fix) | in-model (pure: try candidates, score by badness) | **Built.** `from="auto"` (now the **default**) tries 10 common charsets (Windows-1252, ISO-8859-1/-15, Windows-1251, KOI8-R, Shift_JIS, EUC-JP/KR, GBK, Big5) and keeps the result with the fewest U+FFFD / C1-control "tells". |
| **Double-encoded / recursive mojibake** ("try two passes") | in-model | **Built.** `passes` param (1–8); each pass re-applies the repair and stops early once the text is clean (an over-fix guard rejects a pass that would only dirty already-clean text). |
| Multiple explicit legacy charsets (CJK, Cyrillic, Western) | in-model | **Already covered** — accepts any WHATWG charset label/alias via `encoding_rs`. |
| Strict vs. lossy handling of undecodable bytes | in-model | **Already covered** — `errors` = `replace` (default, U+FFFD) or `strict` (error). Also reports a clear "wrong 'from'" message when a guess can't apply. |
| Client-side / private | in-model | **Already covered** — pure wasm, runs entirely in-browser; CLI is local. |
| **HTML-entity decoding** button | partially in-model but **out of scope** | **Not built** — entity decoding (`&amp;`/`&#233;`) is a distinct concern from charset mojibake and overlaps the existing `url-encode` / HTML tools; listed, not forced in. |
| Inline copy-to-clipboard / preview chrome | in-model (page chrome) | Handled by the shared gizza page chrome (output panel); no tool-specific work. |

## Out-of-model features considered, not built

- Cloud/batch file processing, accounts, API keys, paid tiers — gizza is
  browser-local, no-server, no-account.
- HTML-entity / numeric-entity decoding (scope overlap; see table).

## Tests (all green)

- **Unit:** 14 core tests (Windows-1252 / Latin-1 / Shift_JIS mojibake, smart
  quotes, ASCII passthrough, auto-detect, auto-leaves-clean-text, double-mojibake
  un-nesting, passes clamp, wrong-charset rejection, strict vs replace, unknown
  charset) + drift-guard schema test.
- **Chat block:** `wafer build` validates/instantiates (`encoding_rs` is
  wasm-safe); 4 wafer fixtures pass (`fix-mojibake`, `auto-detect`, `bad-errors`,
  `unknown-charset`).
- **CLI:** `gizza tool charset-transcode` — auto, explicit charset, latin1 alias,
  double-pass, and error paths all behave.
- **Page:** 6 Playwright tests pass incl. the **query-param deep-link**.

## Limitations

- Input arrives as a Rust `&str` (valid UTF-8 characters), so the tool fixes
  *re-decode* mojibake (UTF-8 mis-shown as a legacy charset). It does not ingest
  raw arbitrary byte files (no binary upload surface for text yet).
- `auto` is a heuristic (badness scoring); for ambiguous or non-mojibake text it
  declines to "fix" rather than guessing wrongly, and the user can specify
  `from` explicitly.
