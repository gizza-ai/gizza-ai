# audio-pad-silence — competitor analysis (2026-08-20)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of what each tool exposes — no competitor copy,
branding, or trademarks are reproduced or reused.

Backlog row: `audio-pad-silence` — "Adds a chosen length of silence to the start and/or end
of a clip." (`tools-to-build.csv:1471`, size S, type_hint `ffmpeg`).

## Search + tools skimmed

One search for "add silence to beginning and end of audio online free tool". Four tool
pages were opened; one (melobytes) returned HTTP 403 to the fetcher and was replaced, so
three real, reachable tools were skimmed:

1. **descript.com — add-silence tool page.** A marketing page in front of a full timeline
   editor rather than a one-shot converter: import by drag-and-drop, pick the silence
   effect on a layer, type an exact silence duration. Input formats listed as mp3, wav,
   aac and flac; no start/end position control, no presets, no waveform-placement control
   and no stated file-size limit documented on the page (usage is metered by plan tier in
   media-hours). Four FAQ entries: supported formats, duration customisation, pricing
   tiers, team features.
2. **tembrica.com — add-silence tool.** The most capable of the three. Duration preset
   buttons (0.5 s, 1 s, 2 s, 5 s) plus a custom value, up to 3600 s per insertion and any
   number of insertions; placement by clicking an interactive waveform, which snaps to the
   clip edges for start/end padding; live waveform preview of the pending inserts; undo
   and clear-all; playback with speed control. Input mp3/wav/flac/ogg/m4a up to 1 GB, with
   a warning that files over ~150 MB get slow; output format selectable. FAQ covers
   start/end insertion, middle placement, the duration ceiling, and quality preservation.
3. **elysiatools.com — audio add-silence tool.** A plain form: numeric silence duration
   documented as 0.1 s to 3600 s; a position dropdown with exactly two choices (start or
   end); output format dropdown (mp3, aac, m4a, ogg/vorbis, opus, flac, wav); optional
   sample-rate dropdown (keep original / 44.1 kHz / 48 kHz) and channels dropdown (keep
   original / mono / stereo); 100 MB upload cap. Notably it **cannot** pad both ends in one
   pass — its own FAQ says to run the tool twice. FAQ covers formats, duration range, size
   limit, quality, and the both-ends limitation.

Also cross-checked against the documented behaviour of the ffmpeg filters that back this
tool (`adelay` for a lead-in, `apad=pad_dur=` for a bounded tail), spiked locally before
the descriptor was designed — see "Feasibility spike" below.

## Table-stakes, and where each one landed

| Capability | Competitors | Decision |
| --- | --- | --- |
| Add N seconds of silence to the **start** | all three | **in model** — `start` (seconds, default 2) |
| Add N seconds of silence to the **end** | all three | **in model** — `end` (seconds, default 0) |
| Pad **both ends in one pass** | only implied by the waveform tool; the form-based one explicitly requires two runs | **in model** — two independent fields, one ffmpeg pass; this is the main differentiator |
| Fractional-second durations | 0.1 s floor on one; 0.5 s preset on another | **in model, better** — decimals accepted down to `0.001` (1 ms, the `adelay` resolution); anything smaller is rejected with a message rather than silently rounded to a no-op |
| Duration ceiling of 3600 s per side | two of three state 3600 s | **in model** — `min 0 / max 3600` on each side, matched exactly |
| Preset duration buttons (0.5 s / 1 s / 2 s / 5 s) | one tool ships them | **in model** — five `[[example]]` preset chips (0.5 s lead-in, 1 s both ends, 2 s intro gap, 5 s tail only, IVR prompt to WAV), which is this generator's declarative preset control |
| Output format choice | all three (sets vary) | **in model** — `format` enum mp3 (192 kbps, default), wav, ogg, flac, m4a — the gizza audio-family standard set |
| Multiple input formats accepted | mp3/wav/flac/ogg/m4a | **in model** — page accepts `audio/*`; CLI/chat accept any `audio/*` MIME |
| "Original audio untouched / no quality loss" claim | two tools advertise it | **in model, stated honestly** — the clip's own samples are unaltered, but the file *is* decoded and re-encoded (silence appended to a compressed stream clicks at the seam). The page says so and points at wav/flac to avoid a second lossy generation |
| Stated file-size limit | 100 MB / 1 GB | **in model but lower** — 10 MiB in and out, stated on the page; this tool runs entirely in the browser tab / local runtime, it does not upload to a server |
| Client-side / privacy | one advertises no server upload | **already true** — ffmpeg WebAssembly in the tab; stated in the copy |
| Insert silence in the **middle** at an arbitrary point | one tool (waveform click) | **out of model** — needs an interactive waveform editor and a stateful multi-insert model; the page generator has no waveform control. Documented on the page with the workaround (split with `trim-audio`, pad the pieces) |
| Interactive waveform preview, playback, speed control, undo, clear-all, multi-insert | one tool | **out of model** — editor-surface features, not one-shot-transform parameters |
| Sample-rate override (auto / 44.1 / 48 kHz) | one tool | **deliberately out of scope, not dropped** — `-ar` is trivially feasible, but `blocks/audio-resampler` already owns it; gizza blocks are single-purpose. Page copy routes the user there |
| Channel override (auto / mono / stereo) | one tool | **deliberately out of scope, not dropped** — `-ac` is trivially feasible, but `blocks/audio-to-mono` already owns it; page copy routes the user there |
| Opus / AAC-in-`.aac` output | one tool | **out of scope** — the audio family across gizza standardises on mp3/wav/ogg/flac/m4a; m4a already carries AAC |

## Feasibility spike (before any "out of model" tag)

Ran locally against ffmpeg 7.1.4 on a 3 s lavfi sine, per the "feasibility ≠ model fit" rule:

- `-af "adelay=2000:all=1,apad=pad_dur=1.5"` on a 3.03 s input → **6.53 s** output (exactly
  `start + input + end`). Confirms the two filters compose in one chain and that no
  output-duration guess (`-t`) is needed — `pad_dur` is the *bounded* form of `apad`, unlike
  the unbounded default that forces callers to compute a total length.
- Start-only `-af "adelay=500:all=1"` on the same input → 3.5 s.
- End-only `-af "apad=pad_dur=4"` → 7.0 s.

So both-ends padding, the capability the closest form-based competitor tells users to
achieve with two separate runs, is one filter chain. It went into the descriptor from the
start rather than being deferred.

## Design decisions this drove

- **Two numeric fields, not a position dropdown.** The dropdown model (one duration + a
  start/end selector) is what forces a second run for both ends. Two fields make the
  common case one pass and keep the one-sided case trivial (leave the other at `0`).
- **Both-zero is an error, not a no-op.** With two independent fields the "do nothing"
  state is reachable; returning a silently re-encoded copy would be worse than an error
  that names the fix. The FAQ routes a genuine format-only conversion to `audio-convert`.
- **No slider control.** The generator's `kind = "slider"` needs schema `min`+`max`, and a
  0–3600 range makes drag precision useless for the 0.25–5 s values this tool is actually
  used at. Plain number boxes with placeholders plus the five preset chips are the right
  controls here; the chips cover what the competitor's preset buttons cover.
- **Distinct from `audio-fit-to-length`.** That block pads *or trims* to an absolute target
  duration at one position; this one adds a *relative* amount at both ends and never trims.
  Checked before building (`blocks/audio-fit-to-length/core/src/lib.rs`) — not a duplicate.

## Verification notes

Every advertised value form is exercised end-to-end, not just in argv/unit shape: all five
`format` enum choices as real CLI runs with the output duration probed, start-only /
end-only / both-ends, a fractional (sub-second) value, the exact 3600 s cap and one over on
each side, the both-zero error, the sub-millisecond rejection, a secondary input format
(wav as well as mp3), and a `?start=…&end=…&format=…` deep link on the page plus a
non-default in-page run whose output duration is decoded and asserted.
