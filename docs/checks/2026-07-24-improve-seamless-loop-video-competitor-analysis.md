# seamless-loop-video — competitor analysis (2026-07-24)

Backlog tool type: ffmpeg video transform. Research was performed before
implementation using web search and reachable tool pages. Everything below is
paraphrased; no competitor copy, branding, or assets were reused.

## Duplicate and feasibility check

`blocks/loop-video` repeats or extends a clip with `-stream_loop` and leaves the
original last→first cut untouched. This tool is therefore distinct: it creates
one genuinely loopable cycle by hiding that boundary with a cross-dissolve.

A native ffmpeg spike against `tests/fixtures/tiny-128x128.mp4` proved that
`split` + `trim` + `xfade` can rotate the clip at its midpoint, blend the
original end into the beginning, preserve the 128×128 frame size, and emit an
H.264 MP4. ffmpeg also exposes `acrossfade`, so audio crossfading is in-model.

## Competitors surveyed

| tool | parameters / defaults / formats (paraphrased) | UX patterns and strengths |
| --- | --- | --- |
| [Clip Looper](https://thecliplooper.com/) | MP4/WebM/MOV upload; repeat count defaults to 3; crossfade duration defaults to 1 second; stated browser tier limit 500 MB | Drag-and-drop, explicit process action, numeric repeat and fade controls, local-processing message, short explanation of crossfading |
| [DataChef seamless loop](https://tech-lagoon.com/moviechef/en/movie-seamless-loop.html) | MP4/MOV, stated 10 MB input cap; automatic end-to-start blend; no exposed fade value in the indexed page | One primary upload/convert flow, before/after samples, practical source-shot examples, warns that fixed-camera footage works best |
| [Kapwing crossfade](https://www.kapwing.com/tools/crossfade) | Adjustable dissolve duration; multi-clip timeline; HD export; free exports are described as watermarked | Duration slider, instant preview, timeline editing, transition browser, continued editing after the crossfade |

## Table stakes and fit decisions

| table stake | decision |
| --- | --- |
| Upload common video containers | **In-model:** `Input::Video`; page accepts video files; MP4 and WebM are exercised end to end. Output is broadly compatible H.264 MP4. |
| Adjustable crossfade duration | **In-model:** 0.05–10 second bounded slider, default 1 second, with validation that it stays below half the clip. |
| Smooth end→beginning blend | **In-model:** the source is rotated at its midpoint and the original boundary is cross-dissolved inside the output, leaving a naturally continuous outer boundary. |
| Preserve or blend audio | **In-model:** explicit `remove` (safe default) and `crossfade` choices. The latter requires a source audio stream. |
| Output-quality choice | **In-model:** high/balanced/small H.264 presets (CRF 18/23/28). |
| Presets / worked examples | **In-model:** three page chips for a short clip, ambient background, and audio-preserving case. |
| Clear limits and suitable-source guidance | **In-model:** page states duration, fade, byte, format, re-encode, audio, memory, and ghosting limits; FAQ explains why fixed shots work best. |
| Automatically probe duration | **Out-of-model for the current one-command page bridge:** `build_argv` is called before ffmpeg runs and receives no ffprobe metadata, while `xfade` needs numeric trim/offset values. The exact duration is therefore a required parameter. |
| Timeline trimming, multi-clip editing, transition libraries | **Out-of-scope:** these are editor workflows rather than one focused local transform; existing tools handle trimming and repeating separately. |
| Server rendering, accounts, project history, social publishing | **Out-of-model:** require persistent backend/account infrastructure. |
| AI frame interpolation or motion-aware morphing | **Out-of-model:** requires a model and substantially more compute than the ffmpeg wasm/runtime path. |

## Resulting design

The descriptor exposes `duration`, `crossfade`, `audio`, and `quality`, each
with actionable descriptions. The page uses a crossfade slider, friendly enum
labels, three presets, a worked numerical example, four FAQ accordions, and
explicit limits. The output is one loopable MP4 cycle—not a longer repeated
file—so it complements rather than duplicates `loop-video`.

> Original work only — no competitor copy, branding, or trademarks copied.
