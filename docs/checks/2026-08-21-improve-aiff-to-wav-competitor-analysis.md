# aiff-to-wav — competitor analysis (2026-08-21)

Scan run against the public online "AIFF to WAV" converter set plus the command-line reference
(`ffmpeg`'s wav muxer / PCM encoders) that all of them wrap. All findings are paraphrased from
public documentation and visible option surfaces; **no competitor copy, branding, or trademarks
are reproduced here or on the page.**

## What the tool has to be

"AIFF → WAV" is a crowded, commodity category: browser-upload converter sites, desktop batch
converters, and the DAW/editor export dialogs people already own. They converge on a remarkably
uniform option surface, which is what defines table stakes here:

- upload by file picker **and** drag-and-drop (several also offer a cloud-storage picker),
- WAV as the output container, with the conversion described as "lossless",
- an output **sample rate** selector (a fixed list from telephony 8 kHz up to 96–192 kHz, plus a
  "same as source" choice),
- a **channels** selector (mono / stereo, sometimes "same as source"),
- a **bit depth / audio codec** selector (8/16/24/32-bit PCM, often with float and the G.711
  companded encodings alongside),
- some form of **trimming** (start/end range) and/or a target-quality / target-file-size control,
- and a hard cap on input size or conversion count on the free tier.

The differentiator available to this block is not the option list — it is that the conversion runs
locally with no upload, and that it refuses to silently truncate a 24-bit master (see #4 below,
the one real correctness trap in this category).

## Table-stakes, and where each one landed

| # | Table-stake behaviour | Verdict | Where it landed |
|---|---|---|---|
| 1 | Upload by file picker, with drag-and-drop | **in-model** | page `[[input]] source = "file"`, `accept = "audio/*,.aif,.aiff,.aifc"`; the generated ffmpeg page ships drag-drop and paste-to-upload generically |
| 2 | WAV (RIFF/WAVE) output with a download link | **in-model** | core always muxes `out.wav`; page `format = "audio"` renders `<audio controls>` + download |
| 3 | Output sample rate from a fixed list, plus "same as source" | **in-model** | `sample_rate` enum: `keep` (default) + 8000/16000/22050/44100/48000/88200/96000/192000; `keep` omits `-ar` entirely |
| 4 | Output bit depth / PCM codec choice | **in-model** | `bit_depth` enum: `16`/`24`/`32`/`float32`/`alaw`/`mulaw` → `pcm_s16le`/`pcm_s24le`/`pcm_s32le`/`pcm_f32le`/`pcm_alaw`/`pcm_mulaw`. **Default `24`, always sent explicitly** — ffmpeg's wav muxer otherwise falls back to `pcm_s16le` and silently truncates a 24-bit source. Competitors that expose no depth control inherit exactly that truncation |
| 5 | Mono / stereo channel choice | **in-model** | `channels` enum: `keep` (default) / `mono` (`-ac 1`) / `stereo` (`-ac 2`) |
| 6 | Preserve or strip textual tags | **in-model** | `keep_metadata` boolean (default true) → `-map_metadata 0` / `-map_metadata -1`, into the WAV LIST/INFO chunk |
| 7 | Don't break on files carrying embedded cover art | **in-model** | `-vn` always drops the attached-picture stream, which rides as a video stream and would otherwise fail the audio-only wav mux |
| 8 | Accept the extension variants of the format | **in-model** | `.aif`/`.aiff`/`.aifc` in `accept`; ffmpeg probes the bytes, so the input extension only names the scratch file |
| 9 | State the size limit up front instead of failing mid-convert | **in-model** | 25 MiB input / 50 MiB output caps in the block, both documented in "Limits and edge cases"; the output cap is deliberately higher because 24-bit widening grows a 16-bit source 1.5× |
| 10 | Explain that the conversion is lossless, and when it stops being lossless | **in-model** | page copy + FAQ: identical samples at a matching depth; depth reduction, resampling and downmixing each called out as one-way |
| 11 | One-click presets for the common targets | **in-model** | six `[[example]]` chips: defaults, CD 16-bit/44.1 kHz stereo, 48 kHz video, 32-bit float, mu-law 8 kHz telephony, strip-tags |
| 12 | Friendly option labels rather than raw enum values | **in-model** | `[input.labels]` on all three enums ("Keep source rate", "24-bit integer PCM (default)", "48000 Hz (video)", …) |

### Out-of-model (listed, deliberately not built)

- **Trim / start–end range.** Several competitors pair conversion with a range selector. This
  block is a single-shot container conversion; adding `-ss`/`-to` would duplicate
  `blocks/audio-trim`'s scope and its page surface. Called out in the page's limits section so the
  absence isn't a surprise.
- **Batch conversion / multi-file queues.** The page file input is a single upload and the ffmpeg
  page runs one job at a time; multi-input ffmpeg pages are not buildable in this repo. The CLI
  covers batches from a shell loop, which the FAQ says explicitly.
- **Target-file-size or target-quality sliders.** Meaningless for uncompressed PCM — output size
  is fully determined by depth × rate × channels × duration, all three of which are already
  directly exposed. A slider would only be an indirect way of setting those.
- **Cloud-storage import (Drive/Dropbox-style pickers) and emailed results.** Both require a
  server-side account integration; this page is local-only by design.
- **Cover-art / tag editing.** `keep_metadata` copies or strips tags; it is not a tag editor, and
  WAV has no standard picture chunk to write art into.
- **Non-PCM WAV payloads (e.g. MP3-in-WAV).** Out of the row's intent — `blocks/audio-convert`
  handles lossy targets.

## UX control patterns worth copying

- **"Same as source" as the default for every lossy transform.** The competitor sets that do this
  well make "same as source" the first entry in the rate and channel lists; the ones that default
  to a concrete 44100/stereo quietly resample and downmix everything. `sample_rate = "keep"` and
  `channels = "keep"` follow the former.
- **Preset chips over an options matrix.** Every headline target (CD, video, float, telephony) is
  one click, with the full enum still available underneath.
- **Say what changed, not just "done".** The worked example prints the exact argv the defaults
  produce, so the conversion is auditable rather than opaque.

## Decisions taken into the descriptor

- `bit_depth` defaults to `24` and is **never** omitted from the argv — the single most important
  correctness decision in this block (#4).
- `sample_rate` and `channels` default to `keep`, so the out-of-the-box conversion is genuinely
  lossless and every quality-affecting change is opt-in.
- `keep_metadata` defaults to `true` (preserve by default; stripping is the deliberate act).
- Enum values stay canonical (`44100`, `mulaw`); friendliness lives in `[input.labels]` so deep
  links, CLI args and chat arguments share one vocabulary.

Sources consulted: public option surfaces of the mainstream browser-based AIFF→WAV converters,
FFmpeg's WAV muxer and PCM encoder documentation, and the AIFF/AIFF-C and RIFF/WAVE format
specifications.
