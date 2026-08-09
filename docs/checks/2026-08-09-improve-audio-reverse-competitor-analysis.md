# audio-reverse — competitor analysis (2026-08-09)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of publicly visible functionality. No competitor
copy, branding, or trademark is reproduced or reused anywhere in the tool.

## Scope

Backlog row: `audio-reverse — "Reverses an audio clip." (type hint: ffmpeg, `areverse`)`.

Duplicate check first: `ls blocks/ | grep -i audio` lists 45 audio-touching blocks, and
`grep -rl areverse blocks/` matches `trim-audio`, `audio-fade`, `audio-fit-to-length`,
`audio-ringtone`, `audio-edge-silence-trimmer`, `video-audio-fade`. In every one of those
`areverse` is an INTERNAL idiom (reverse → apply a head-anchored filter → reverse back) used
to fade/pad/trim the tail without knowing the clip duration; none of them exposes
playing the audio backwards as a user-facing capability, and none produces a reversed file.
`gif-reverse` is the closest named sibling and is image-only. So this is not a duplicate.

## Competitors reviewed

1. **audiotrimmer.com — Online Mp3 Reverser.** Single upload + one "reverse" action. Accepts
   mp3, wav, wma, ogg, m4r, m4a, aac, amr, flac, aif. 50 MB cap. No format selector, no other
   options.
2. **audioalter.com — Reverse.** Drag-and-drop upload, no parameters at all. Lists mp3, wav,
   flac, ogg as accepted. 50 MB cap.
3. **mp3cut.net — Audio Reverser.** Three-step upload → auto-process → download. Accepts mp3,
   m4a, m4r, flac, wav. Returns the file in its ORIGINAL format ("no conversion"). Adds
   Drive/Dropbox/URL import; free-tier size cap not published (paid tier 10 GB).
4. **audio.pi7.org — Reverse Audio.** The most featureful of the set: upload plus an OUTPUT
   FORMAT selector (mp3 default, wav, m4a, ogg vorbis, flac). Accepts mp3, wav, aac, m4a, ogg,
   flac, opus, aiff, caf, and also video containers (mp4/mkv/mov/webm) by extracting the audio.
   States it runs ffmpeg compiled to WebAssembly locally with no upload. Points users at
   separate cutter / loop / pitch tools for anything beyond reversal.
5. **freebeat.ai / reverseplay.org — Reverse Audio.** Upload or record a clip in the browser,
   preview, download (WAV output). No parameters beyond the recording step.

## Table stakes → decisions

| Table stake | In model? | Decision |
| --- | --- | --- |
| Reverse the whole clip, sample-exact | yes | Core capability — ffmpeg `areverse`. |
| Broad input format acceptance (mp3/wav/ogg/flac/m4a/aac/aiff/…) | yes | `Input::Audio` + `AssetKind::Audio` accept the `audio/*` MIME class; the page file input uses `accept="audio/*"`. Decoding is whatever ffmpeg supports. |
| Output format selector (mp3 / wav / m4a / ogg / flac) | yes | `format` enum, default `mp3`, matching the audio-family standard set and pi7's exact list. |
| Preview the result before downloading | yes | Page `format = "audio"` renders `<audio controls>` plus a download link — already generic platform behaviour. |
| Local / private processing, no upload | yes | Page runs ffmpeg-wasm in the tab; stated plainly in the page copy. |
| Stated size limit | yes | 10 MiB in/out cap, documented on the page (competitors publish 50 MB). |
| Return the file in its ORIGINAL format (mp3cut) | partly | The `format` enum covers the five common containers; there is no "same as input" passthrough because reversal necessarily re-encodes (the decoded samples change). Documented on the page instead of silently differing. |

## Beyond table stakes (added because it is cheap and in model)

- **`mode`** — `reverse` (default), `forward-reverse`, `reverse-forward`. The second and third
  emit the clip and its reversal back to back (a palindrome / boomerang), which is the standard
  way to build reverse-cymbal swells and riser transitions — the exact use case audiotrimmer
  advertises in prose but does not offer a control for. Implemented with
  `[0:a]asplit=2[a][b];[b]areverse[r];[a][r]concat=n=2:v=0:a=1[out]` (order swapped for
  `reverse-forward`), which runs identically in native ffmpeg and `@ffmpeg/core`
  (same `-filter_complex` + `-map [out]` shape `audio-bleep-censor` already ships).
- **`[[example]]` preset chips** — "Play it backwards", "Reverse cymbal riser", "Lossless WAV
  reverse" — competitors ship no presets, but chips are the platform's declarative preset
  answer and make the two non-obvious modes discoverable.
- **`[input.labels]`** — friendly `<select>` labels for both enums (MP3/WAV/… and
  "Reversed only" / "Original then reversed" / "Reversed then original").

## Out of model here (listed, not built)

- **Record-a-clip in the browser** (freebeat, reverseplay): the page input is a file upload;
  there is no microphone-capture control in the generic page runtime. Platform gap, not a
  tool gap.
- **Reverse only a selected region / trim-then-reverse** (mp3cut's editor): already covered by
  chaining `trim-audio` → this tool. Adding start/duration here would duplicate `trim-audio`'s
  parameters.
- **Video input with automatic audio extraction** (pi7): the descriptor takes a single
  `Input::Audio` source and the page accepts `audio/*`. `extract-audio-from-video` covers the
  first hop; chaining is the model-fit answer.
- **Cloud imports (Drive/Dropbox)** (mp3cut): no OAuth surface in this repo. URL input already
  exists on the chat/CLI surfaces via `url=`.
- **Bitrate / quality control**: lossy output is fixed at 192 kbps across the whole audio block
  family; `wav` and `flac` are the lossless escape hatches. Consistency beats a per-tool knob.
- **50 MB / 10 GB size ceilings**: gizza's envelope cap is 10 MiB in and out. Stated on the page.

## Result

Every table stake above is either in the descriptor or in the out-of-model list; none was
dropped silently. Implemented params: `mode` (enum, default `reverse`), `format` (enum,
default `mp3`).
