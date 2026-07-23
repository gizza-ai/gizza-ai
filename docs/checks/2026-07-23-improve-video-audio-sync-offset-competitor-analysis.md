# Competitor analysis — video-audio-sync-offset (2026-07-23)

**Tool function:** shift a video's audio track earlier or later relative to the
picture by a fixed time offset to correct lip-sync drift. The picture is
stream-copied (lossless); only the audio timeline moves.

## Scan (paraphrased — no competitor copy/branding reproduced)

Searched "fix audio video sync offset online tool delay audio milliseconds lip
sync". Top 3 real competitors skimmed:

1. **ImageOnline — Sync Audio & Video.** Upload a video, use +/− stepper buttons
   to set a delay in **seconds** (e.g. +0.5 s when audio is early), press a
   "sync" button. Browser-local processing (files stay on device). One offset
   value; direction is expressed via the sign / +/− buttons.
2. **OnlineConverter — Sync Audio and Video.** Offers two modes: **Delay Audio**
   or **Delay Video**, with the amount entered in **seconds** (0.5, 1.6, …).
   "Delay video" is just the negative-offset direction (advancing the audio),
   so a single signed offset covers both.
3. **OBS Studio — Sync Offset (Advanced Audio Properties).** Fine-tunes an
   audio/video sync offset in **milliseconds**; guidance suggests starting at
   100–200 ms and adjusting. Bluetooth-mic setups commonly need ≈ −200 ms.

## Table-stakes params, defaults, UX

| Feature | Competitor pattern | In/out of model | Decision |
|---|---|---|---|
| Offset amount | single numeric field (s or ms) | in-model | `offset` number, required |
| Direction (audio later/earlier) | +/− sign, or "delay audio / delay video" | in-model | signed `offset`: `>0` delays audio, `<0` advances it (covers "delay video") |
| Unit ms vs seconds | competitors split (OBS=ms, web tools=seconds) | in-model | `unit` enum `ms|seconds`, default `ms` (name/description are ms-based) |
| Preset offsets (Bluetooth −200 ms, +0.5 s) | steppers / typical-value hints | in-model | `[[example]]` preset chips |
| Keep picture untouched | implicit | in-model | `-c:v copy` (lossless) |
| Browser-local, no upload | stated selling point | in-model | ffmpeg runs in the page/CLI, nothing uploaded |
| **Auto-detect the drift** (AI frame analysis) | OutOfLipSync AI aligner | **out-of-model** | listed, not built — needs ML frame/audio analysis; the existing `video-audio-sync-offset-finder` measures an offset between two files, but auto lip-detection is out of gizza's pure-Rust+ffmpeg model |
| Stretch/resample to fix *drift-over-time* | pro NLE feature | **out-of-model** | listed, not built — a constant offset is one gain of PTS; progressive drift needs `atempo`/rubberband time-scaling, a different tool |

Every in-model table-stake is in the descriptor (`offset`, `unit`, sign =
direction) or the page (`[[example]]` chips). Out-of-model items are listed
above, not built.

## Sign convention shipped

`offset > 0` → audio is **delayed** (plays later; fixes audio that runs *ahead*
of the picture) via `adelay`. `offset < 0` → audio is **advanced** (plays
earlier; fixes audio that *lags*) via `atrim` + `asetpts=PTS-STARTPTS`. The
picture is always `-c:v copy`; only the audio is re-encoded (AAC, or Opus for
WebM). Range ±60 000 ms (±60 s); 0 rejected as a no-op.
