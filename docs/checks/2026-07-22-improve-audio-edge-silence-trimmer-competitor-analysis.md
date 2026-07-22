# audio-edge-silence-trimmer — competitor analysis (2026-07-22)

Function: remove ONLY the leading and trailing silence below a threshold, leaving the
body (internal pauses) untouched. Distinct from `audio-silence-remove` (removes every
gap incl. middle, `stop_periods=-1`) and `audio-pause-shortener` (shrinks internal
pauses). This is the "auto-trim edges" variant.

## Competitors skimmed (top 3, paraphrased — no copy/branding reproduced)

1. **audioeditor.org — Silence Trimmer.** Removes long silence from the beginning and
   end only, keeps a small natural pad at each edge so the cut isn't abrupt. Runs locally
   in the browser. Threshold-based detection. Output keeps original format.
2. **Notevibes — Silence Remover (Auto-Trim mode).** Three modes; the Auto-Trim mode
   cleans just the start and end. In-browser, no upload. Adjustable sensitivity.
3. **WuTools — Silence Trimmer.** Auto-detects and removes silence from the beginning and
   end by analysing the waveform and trimming quiet portions below a threshold setting.
   Multiple audio formats.

(Also seen: Kapwing one-click AI trim; KlipTools waveform trimmer; Submind podcast
auto-trim, no sign-up.)

## Table-stakes params, defaults, UX

| Capability | Competitors | Our decision |
|---|---|---|
| Silence threshold (dB) | yes, adjustable sensitivity (~ -40…-60 dB typical) | `threshold_db` number, max 0, default **-50** |
| Keep a natural pad at each edge | yes (small pad so cut isn't abrupt) | `pad` seconds kept at each edge, default **0.1** |
| Output format choice | mp3/wav/m4a etc. | `format` enum mp3/wav/ogg/flac/m4a, default mp3 |
| Edges only, body untouched | the defining feature | `silenceremove` + `areverse` idiom, no `stop_periods` |
| In-browser, no upload, no account | universal | native (wasm + page ffmpeg) |
| Preset chips (aggressive/gentle) | some ship sensitivity presets | `[[example]]` chips: default, gentle (-60/0.25), aggressive (-40/0) |

## UX patterns matched
- Slider for threshold (bounded numeric range) + preset chips (gentle/aggressive).
- Real worked example on the page; FAQ accordions; stated limits (10 MiB, edges only).

## Out-of-model (listed, not built)
- AI/waveform visual trim-point editor (needs a waveform-render/UI surface + interaction).
- Batch/multi-file processing (single-input ffmpeg model, one upload).
- Loudness-aware detection presets tied to speech models.

Everything table-stakes lands in the descriptor above; nothing dropped silently.
