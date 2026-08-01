# srt-to-plaintext — competitor analysis (2026-07-29)

Scan done BEFORE implementation. Paraphrased only — no competitor copy/branding/trademarks
reproduced. Goal: capture table-stakes params/defaults/UX for an SRT → plain-text
(transcript) extractor, tag each in-model vs out-of-model for the gizza model
(browser-local wasm, no account, no server).

## Competitors skimmed

1. **miniwebtool — SRT to TXT** (`miniwebtool.com/srt-to-txt/`)
2. **subtitlekit — SRT to TXT** (`subtitlekit.com/en/srt-to-txt/`)
3. **subtitletoolkit — how-to convert SRT to TXT** (`subtitletoolkit.tools/.../how-to-convert-srt-to-txt/`)

(Also surfaced: TimeWipe, SubtitleOps, MacParakeet, Sozai — same feature envelope.)

## Feature / option landscape (paraphrased)

| Capability | Seen at | Default | Our decision |
| --- | --- | --- | --- |
| Drop cue numbers + timing lines + blank lines | all three | always on | **Core behavior** (always) |
| Strip HTML/formatting tags (`<i>`, `<b>`, `<font>`) | miniwebtool, subtitlekit, toolkit | on | **in-model** — `strip_tags`, default on (also strips `{...}` ASS override tags) |
| "One line per cue" / join a cue's internal line breaks | subtitlekit | off | **in-model** — folded into `layout` |
| Merge into flowing paragraphs | miniwebtool ("Merged Sentences") | — | **in-model** — `layout = paragraph` |
| Preserve original per-cue segmentation | toolkit (default) | — | **in-model** — `layout = blocks` |
| Remove sound-effect / bracketed descriptions `[door slams]`, `(applause)`, ♪ | miniwebtool | off | **in-model** — `remove_sound_effects`, default off |
| Remove speaker labels `NARRATOR:` / `- JOHN:` | miniwebtool | on (there) | **in-model** — `remove_speaker_labels`, default **off** (heuristic can over-strip; conservative) |
| Deduplicate consecutive repeated cues (rolling auto-captions) | toolkit (manual step) | off | **in-model** — `dedupe`, default off |
| Timestamped-text output mode (`[MM:SS] line`) | miniwebtool | — | **considered, rejected** — inverse of a *plaintext* extractor; `srt-shift`/`srt-to-vtt` already own timed formats |
| JSON export (index/start/end/text) | miniwebtool | — | **out-of-model here** — a distinct "srt-to-json" shape, not this tool's job |
| Word/char/subtitle-count + duration stats | miniwebtool | — | **out-of-model** — `text-statistics`/`word-count` blocks already cover this |
| File drag-drop / upload | all | — | Page already provides paste + generic upload; core takes text |
| Copy / download output | all | — | Generic page gives Copy + Download for `format="text"` |
| Runs locally, no upload, no signup | all | — | **Native** to the gizza model |

## Model-fit notes

- Every table-stakes cleaning toggle is pure text transformation → fully in-model.
- WebVTT tolerance (period-separator timings, `WEBVTT`/`NOTE`/`STYLE` header lines) added for
  robustness even though the tool is SRT-first; the sibling `srt-to-vtt` owns real conversion.
- Speaker-label removal ships **off by default** because the heuristic (leading `NAME:`) can
  clip legitimate dialogue like `Time: 5pm` — documented on the page.
