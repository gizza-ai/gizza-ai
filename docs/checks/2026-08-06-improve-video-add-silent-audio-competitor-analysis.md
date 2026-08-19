# video-add-silent-audio — competitor analysis (2026-08-06)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are **paraphrased**; no competitor copy, branding, or trademark is reproduced.

## Function under study

Take a video that carries **no audio stream** and give it one — a track of pure digital
silence — so uploaders, editors, and players that require an audio stream stop rejecting or
mis-handling the file. The picture must not be re-encoded.

## Duplicate / viability check

`ls blocks/ | grep -iE 'silent|audio|video'` + skiplist grep:

| Candidate | Verdict |
| --- | --- |
| `blocks/video-mute` | **Opposite** function: `-c:v copy -an` **strips** the audio track. This tool **creates** one. Not a dup. |
| `blocks/video-audio-bitrate-set` | Re-encodes an **existing** audio track at a chosen bitrate. Errors out on a video with no audio; never synthesizes one. Not a dup. |
| `blocks/video-audio-track-selector` | Picks among **existing** tracks. Not a dup. |
| `docs/tool-skiplist.txt`: `add-audio-to-video`, `video-mux-audio`, `still-image-audio-video` | Skipped because they need **two media inputs** (video + a user-supplied audio file). This tool needs **one** — the silence is generated inside the filtergraph by `anullsrc`, so it fits the single-input ffmpeg dispatch model. **Viable, and explicitly not the same shape as those skips.** |

Feasibility spike (real ffmpeg 7.1.4, against the repo's committed fixtures) confirmed the
single-input form before any code was written:

```
ffmpeg -i in.mp4 \
  -filter_complex "anullsrc=channel_layout=stereo:sample_rate=48000[silence]" \
  -map 0:v -map "[silence]" -c:v copy -c:a aac -b:a 128k -shortest out.mp4
```

`tiny-128x128.mp4` (video-only) → mp4 with `h264` copied + a 48 kHz stereo AAC track;
`volumedetect` reports `max_volume: -91.0 dB` (digital silence). Verified the same for
mov (AAC) and webm (Opus), for mono and for a "keep the existing track" mapping.

## Competitors reviewed

Direct "add a silent track" web tools are thin — the function is mostly documented as an
ffmpeg recipe, plus adjacent "add audio to video" editors that require the user to supply an
audio file. Three real, reachable references were reviewed:

### 1. Long-form ffmpeg recipe article (ourcodeworld, "silenced audio track in FFMPEG")

- Two-step workflow: render a short silent file with `-f lavfi -t 1 -i anullsrc=cl=mono`,
  then mux it into the video with `-af apad -shortest`.
- Explicitly distinguishes *no audio stream at all* from *an existing but muted stream* —
  the same distinction our page copy has to make.
- Exposes channel layout (`cl=mono`) as the one knob; leaves sample rate/bitrate to codec
  defaults.
- **Table-stakes taken:** channel-layout choice, `-shortest` bounding, container-appropriate
  audio codec, and clearly stating which of the two "silent video" cases is being fixed.

### 2. Single-command recipe page (rickmakes, "add silent audio track to a video file")

- One command: `-f lavfi -i anullsrc -vcodec copy -acodec aac -shortest`.
- Video stream copy is the non-negotiable part — the picture must not be re-encoded.
- No sample rate, channel or bitrate options at all.
- **Table-stakes taken:** lossless `-c:v copy`, AAC for mp4-family output, one-shot operation
  with zero required configuration (our tool must work with every field left at its default).

### 3. Browser-local "add audio to video" tool (textground)

- Runs ffmpeg-wasm in the tab; nothing uploaded — same privacy model we ship.
- Controls: replace-vs-mix mode for the existing audio, a volume slider, duration handling
  (match video / shortest), and an output-format selector (mp4 H.264+AAC, webm VP8+Opus).
- UX: drag-and-drop, live duration/resolution readout, explicit success state.
- **Table-stakes taken:** an explicit decision for what happens to an **existing** audio
  track (replace vs keep), duration bounded to the video, output codec that matches the
  container, and browser-local processing with no upload.

Also skimmed for parameter vocabulary: a browser-local silence generator (offers duration,
sample rate and bit depth for standalone silent audio files) and a general ffmpeg
"add audio to video" course page (mapping flags `-map 0:v -map 1:a`, `-c:v copy`, `-shortest`,
and the note that a video that already has audio is either replaced or given a second track).

## Gap list → decisions

| Capability (competitor) | Fit | Decision |
| --- | --- | --- |
| Lossless picture (`-c:v copy`) | in-model | **Built** — always; the video stream is never re-encoded. |
| Silence bounded to video length (`-shortest`) | in-model | **Built** — always on; `anullsrc` is infinite otherwise. |
| Channel layout (mono/stereo) | in-model | **Built** — `channels` enum, default `stereo`. |
| Sample rate choice | in-model | **Built** — `sample_rate` enum (22050/44100/48000), default `48000`. |
| Audio bitrate choice | in-model | **Built** — `bitrate` enum (32/64/96/128/192 kbps), default `128`. Silence costs almost nothing at 32–64 kbps; 128 is the universally accepted default. |
| Behavior when the video **already has** audio | in-model | **Built** — `existing_audio` enum: `replace` (default; output has exactly one silent track) or `keep` (existing track kept, silence added as a second track). Competitors all had to answer this question; ours answers it explicitly instead of silently muting. |
| Container-appropriate codec (AAC vs Opus) | in-model | **Built** — webm → `libopus` (forced 48 kHz, the only rate Opus encodes), everything else → `aac`. Output keeps the input container. |
| One-click presets | in-model | **Built** — `[[example]]` chips: platform-safe default, mono voiceover slot, 44.1 kHz editor-friendly, keep-existing-audio. |
| Friendly `<select>` labels | in-model | **Built** — `[input.labels]` on all four enums. |
| Volume slider / mix levels (textground) | out-of-model *for this tool* | Not built — the added track is silence by definition; a level control is meaningless. Mixing a **user-supplied** audio file needs two media inputs (already skiplisted as `add-audio-to-video`). |
| Supplying your own audio file | out-of-model | Not built — two media inputs; see the skiplist entries above. |
| Output-format conversion (mp4 ⇄ webm) | considered, rejected | The point of this tool is a lossless remux; transcoding the picture to change container would defeat it. `blocks/video-transcode` / `blocks/video-to-h264` cover conversion. |
| Live duration/resolution readout before running | considered, rejected | Page-platform work, not tool work; the generated page already shows the decoded result with native controls. |
| Server-side batch / accounts / watermark-free tiers | out-of-model | Not built — gizza is browser-local, no account, no server. |

## Copy / UX notes taken into the page

- State plainly which problem is being fixed (uploader rejects an audio-less file) and the
  distinction between *no audio stream* and *a silent existing stream*.
- Show a worked example with a named input and the resulting file name.
- State the limits on the page: 25 MB in/out cap, container preserved, Opus is always 48 kHz,
  and that `keep` re-encodes the existing track.
- No competitor copy or naming was reused; all page text is original.
