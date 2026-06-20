# basic-auth-header-generator — competitor analysis (2026-06-20)

Twenty-first `/create-next-tool` backlog pick (this iteration first skiplisted
video-to-frames [dup of video-frame-extract] and the audio-only family
audio-convert/normalize/silence-remove/to-mono/volume-adjust [no Input::Audio
kind]). Pure-Rust (base64) text tool, all 3 surfaces. Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| dev "basic auth header" generators (debugbear, mixedanalytics, various) | base64(user:pass) → header value; copy button | capabilities |
| API-client helpers | output the value or the full `Authorization:` line; sometimes a curl snippet | capabilities |

## Gap diff vs our tool
Our tool: `Basic ` + base64(`username:password`) per RFC 7617; `full_header`
toggle for the whole `Authorization: Basic …` line; rejects a colon in the
username; password may be empty; UTF-8 safe. Covers the standard feature set and
adds the full-line option.

**At parity — nothing material to add.** Notes:
- Pure-local: credentials are never sent (a privacy edge over server-side
  generators).
- Documents that base64 is encoding, not encryption (use over HTTPS).

**In-model gaps considered, deferred (minor):**
- **curl snippet** output (`curl -H "Authorization: Basic …"`) — a third output
  mode; trivial.
- **Bearer / API-key header** variants — those are sibling tools, not this one.

**Out-of-model:** clipboard "copy" button (the page already shows the value to
copy); credential storage.

## Tested
unit (6: value form, full-header form, empty-password ok, missing-username error,
colon-in-username error, unicode password round-trip) + drift-guard · wafer
fixtures (1) · `wafer build` · wasm-pack web · generator · CLI (both output forms)
· Playwright page + query deep-link (2 tests, incl. full_header).

> Original work only — no competitor copy, branding, or trademarks copied.
