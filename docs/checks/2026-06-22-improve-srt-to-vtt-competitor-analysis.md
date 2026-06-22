# srt-to-vtt — competitor analysis & improvement snapshot (2026-06-22)

## Tool
`blocks/srt-to-vtt` — convert subtitles between SubRip (`.srt`) and WebVTT
(`.vtt`). Pure-compute (no I/O), three surfaces: chat skill, CLI, standalone
page. `direction` = `auto` (default, detects input format) | `srt-to-vtt` |
`vtt-to-srt`.

## Top competitors surveyed

| Competitor | Bidirectional | Auto-detect | Browser-local (no upload) | Batch / multi-file | File upload + download | Extra formats |
| --- | --- | --- | --- | --- | --- | --- |
| TechSmith VTT⇄SRT | yes | implicit | partial | no | yes | srt/vtt only |
| Subtitle Batch Tool | yes | — | yes (local) | yes (≤20 files) | yes | srt/vtt |
| VEED subtitle converter | one-way pages | — | no (server) | no | yes | many |
| HappyScribe convert SRT→VTT | one-way | — | no (server) | no | yes | many |
| GoTranscript subtitle converter | many↔many | — | no (server) | no | yes | srt/stl/scc/ass/ttml/sbv/… |

## Capability diff & gap ranking (fit-to-model)

**Covered / at-or-above parity (in gizza's pure-text model):**
- **Bidirectional conversion** SRT⇄VTT — done (`direction` enum).
- **Auto-detect the input format** — done (`auto` default via `detect_is_vtt`,
  the `WEBVTT` signature). Several competitors only offer fixed one-way pages;
  auto-detect is a parity-plus.
- **Browser-local privacy / offline** — done; the page runs the wasm conversion
  entirely client-side, nothing is uploaded (matches the best-in-class
  "processing happens locally" competitors, beats the server-side ones).
- **Format-correctness details** competitors gloss over but we handle:
  comma↔period timestamp separator, add/strip the `WEBVTT` header block
  (incl. `Kind:`/`Language:`/title metadata), expand WebVTT short `MM:SS.mmm`
  form to `HH:MM:SS,mmm`, drop WebVTT cue settings (`line:90%` etc.) when
  producing SRT, preserve cue numbers/text verbatim and `\r\n`/`\n` line
  endings. These are encoded as unit tests.

**Out-of-model gaps (page driver / runtime limitations — NOT built, logged per
the skill's rules):**
- **File upload + download of `.srt`/`.vtt` files.** The pure-tool page is a
  text field (paste in, copy out); there is no `AssetKind` for arbitrary text
  files on the page. Users paste/copy instead. Out of model.
- **Batch / multi-file conversion (e.g. 20 files at once).** The page takes a
  single text input; multi-file upload isn't a supported page surface. Out of
  model.
- **Other formats (ASS/SSA, SBV, SCC, STL, TTML/DFXP, SMI, LRC, CSV …).** The
  backlog item is specifically SRT⇄VTT; the richer many-to-many converters are
  separate tools. Not built here (would be distinct backlog entries), not a
  defect of this tool.

## Copy / UX / visual

- Page copy (`page/content.md`) added: what-it-does, how-to-use, an SRT-vs-VTT
  difference table, a worked SRT→VTT example, and an FAQ (which format to use,
  timing unchanged, styling/positioning dropped, privacy, expected input).
- SEO `meta.toml`: title/description/tags target both "srt to vtt" and
  "vtt to srt" intents; `output_label = "Converted subtitles"`.
- No competitor copy, branding, or trademarks were copied.

## Verification (all surfaces)

- `cargo test --workspace`: 18 tests pass (13 core incl. round-trip, CRLF,
  header-strip, short-form expansion, cue-setting drop, auto-detect; 5 block
  incl. the **drift-guard schema test**).
- `wafer build`: chat `block.wasm` validates + instantiates (300.3 KiB).
- `wasm-pack build … web`: page wasm built.
- CLI (`gizza tool srt-to-vtt`): auto SRT→VTT, forced VTT→SRT, and bad-direction
  error all behave correctly.
- Playwright (`tool-page-srt-to-vtt.spec.ts`): 4 tests pass — auto-detect
  SRT→VTT, VTT→SRT via select, forced srt-to-vtt, and query-param deep-link
  prefill+compute.

## Sources

- [TechSmith VTT⇄SRT](https://www.techsmith.com/tools/vtt-to-srt/)
- [Subtitle Batch Tool](https://subtitlebatchtool.com/)
- [VEED subtitle converter](https://www.veed.io/tools/subtitle-converter/srt-to-vtt)
- [HappyScribe convert SRT to VTT](https://www.happyscribe.com/tools/convert-srt-to-vtt)
- [GoTranscript subtitle converter](https://gotranscript.com/subtitle-converter)
