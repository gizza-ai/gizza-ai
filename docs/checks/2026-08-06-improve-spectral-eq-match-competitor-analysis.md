# spectral-eq-match — competitor analysis (2026-08-06)

Scan run BEFORE implementing, per the create-next-tool loop. All competitor descriptions below are
**paraphrased observations of capability**, never copied copy, branding, or trademarks. Out-of-model
items are listed only — they are not built.

## Scope decision (why this is a PURE tool, not an ffmpeg tool)

An "EQ match" product normally takes **two audio files** (the track to fix + a reference) and renders
a corrected audio file. gizza's ffmpeg dispatch model is **single-media-input**
(`block-utils` `dispatch_ffmpeg` takes one input, the page file driver takes one upload), which is
exactly why `mix-audio` and `audio-crossfade` are already on `docs/tool-skiplist.txt`. A two-audio-file
renderer therefore has no viable surface here.

The viable in-model framing, and the one built: the user supplies **their track's measured per-band
levels** and a **reference** — either pasted reference band levels or a built-in target curve preset —
and the tool derives the **corrective per-band gains**, a ready-to-paste **ffmpeg `equalizer` /
`firequalizer` filter chain**, and a **loudness (dB) offset**. Pure compute, deterministic, works on
all three surfaces (chat schema, CLI, page).

### Not a duplicate of an existing block

| existing block | what it does | why this is different |
| --- | --- | --- |
| `blocks/parametric-eq` | ffmpeg: applies 3 user-chosen peaking bands to one audio file | user must already know the gains; no reference, no derivation |
| `blocks/audio-eq` | ffmpeg: applies bass/mid/treble shelves to one audio file | fixed 3-band tone control, no reference |
| `blocks/loudness-matched-ab-prep` | decodes two files, levels them by LUFS, re-encodes | loudness only, no spectrum/EQ derivation |
| `blocks/loudness-spec-compliance` | measures one file's LUFS/dBTP/LRA vs a delivery spec | measurement + verdict, never derives an EQ curve |
| `blocks/video-audio-loudness-compare` | reports the loudness gap between two recordings | loudness gap only, no spectrum |

None of them derives a **corrective curve from a measured/target spectrum pair**. Not a dup; built.

## Competitors reviewed (4)

### 1. FabFilter Pro-Q 4 — EQ Match (plugin, documented help page)
- Workflow: analyse the plug-in input, pick a reference source, press match, then finish to commit
  the generated bands.
- Reference sources: a saved spectrum, the live plug-in input, another instance / side-chain, or a
  loaded audio file.
- Spectrum is time-averaged during analysis; roughly 30 s of material is suggested for a stable
  average.
- A **customization slider trades band count against smoothness** — more bands = finer matching,
  fewer bands = broader tonal matching. The default picks a band count automatically.
- Matching resolution follows the analyser resolution setting; a higher resolution is recommended for
  low-frequency detail.
- Limits: both input and reference need live signal detection before matching is enabled.

### 2. AudioWrench — Match EQ (browser tool)
- Parameters: **Amount** (scales the learned curve), **Maximum boost/cut** (guards unsupported
  frequency regions), **Spectral smoothing** (favours broad tonal balance over narrow resonances),
  **FFT quality** (speed vs frequency resolution), **Output gain**.
- Users can inspect the measured difference and the resolved correction curve before exporting.
- Monitoring: target / processed / comparison, with bypass.
- I/O: WAV and MP3 in, WAV or MP3 out at the source sample rate and channel count, frame-count
  preserved.
- Stated limits (paraphrased): it cannot reproduce dynamics, stereo image, distortion, ambience or
  arrangement; big boosts can surface noise; the two pieces of material must be comparable and
  representative; spectral similarity does not imply matching dynamics, space or perceived quality.
- FAQ topics: how the matching works, how it differs from a manual/dynamic EQ, how long and how loud
  the material should be, what it can and cannot copy, and upload/privacy.

### 3. Accentize SpectralBalance (plugin)
- Reference-based spectral matching with **static and dynamic modes**, driven by learned models.
- Ships **workflow presets** (dialogue/ADR/podcast oriented); tuned for speech rather than music.
- Continuously adapts the signal toward a desired frequency distribution rather than baking a fixed
  curve.
- Public page does not publish numeric parameter ranges.

### 4. Neural Analog — Match EQ / matchering (browser tool)
- Workflow: upload a track, then either **choose a preset** or supply a reference song.
- Wide input format list; a 50 MB file cap; export as WAV.
- **Loudness handling is explicit**: manual gain plus LUFS-aware targeting for streaming platforms.
- A/B preview of original vs processed.
- Positions the match as context-aware for the specific target/reference pair rather than a fixed
  curve.

## Table stakes → where each one landed

| table stake | seen at | verdict | where it landed |
| --- | --- | --- | --- |
| Derive a correction curve from a measured + reference spectrum pair | all 4 | in-model | core `match_eq`, the whole tool |
| **Amount** — scale the derived correction | AudioWrench, Pro-Q (implicitly) | in-model | `amount` param, 0–100 %, slider |
| **Maximum boost/cut** limit | AudioWrench | in-model | `max_gain_db` param, 0–24 dB, slider (symmetric, as AudioWrench presents it) |
| **Spectral smoothing** (broad balance over narrow resonances) | AudioWrench, Pro-Q ("fewer bands = broader") | in-model | `smoothing` param, 0–4 neighbouring-band radius, slider |
| Band count / resolution trade-off | Pro-Q customization slider | in-model | the user's own band list defines resolution (10-band octave, 31-band third-octave, anything); `smoothing` broadens it |
| **Reference presets instead of a reference file** | Neural Analog, Accentize | in-model | `target_curve` enum: `reference`, `flat`, `pink`, `bright`, `warm`, `speech` |
| **Loudness / output gain, LUFS-aware** | Neural Analog, AudioWrench | in-model | `track_lufs` + `target_lufs` → reported offset + a `volume=<x>dB` stage in the chain |
| Separate tone matching from level matching | Pro-Q, Neural Analog | in-model | `tone_only` boolean (default on) — removes the broadband offset so EQ is tone-only |
| Band **Q**/bandwidth of the corrective filters | Pro-Q (band shape) | in-model | `q` param, 0.1–10 |
| Inspect the measured difference + resolved curve before committing | AudioWrench, Pro-Q | in-model | `output=report` prints per-band track / target / raw diff / smoothed / final gain |
| Export the result | all 4 (as audio) | partly in-model | exported as an ffmpeg **command** (`output=ffmpeg` / `firequalizer`) and as **CSV** — the audio render itself is out-of-model, see below |
| Reference and track measured at different band sets | implied by "load any file" | in-model | log-frequency interpolation of the reference onto the track's band frequencies |
| Stated limits (spectral match ≠ dynamics/stereo/space) | AudioWrench | in-model (copy) | "Limits and edge cases" + FAQ on the page |
| Preset one-click starting points | Neural Analog, Accentize | in-model (UX) | three `[[example]]` chips on the page |

## Out-of-model — listed, not built

- **Rendering the corrected audio in the tool.** Needs two audio inputs (track + reference) plus a
  render; gizza's ffmpeg dispatch is single-input, which is the same blocker that skiplisted
  `mix-audio` / `audio-crossfade`. Mitigation: the tool emits the exact ffmpeg command so the user
  renders locally in one paste.
- **Measuring the spectrum from an uploaded audio file.** Same single-input/second-file problem for
  the reference side, and a full FFT analyser with playback monitoring is a different tool.
- **A/B monitoring, bypass, before/after playback.** Requires audio playback of two rendered
  streams; there is no audio-output surface for a pure text tool.
- **FFT quality / analyser resolution.** The analysis happens upstream in whatever meter produced the
  band levels; this tool consumes the numbers, so an FFT-size control would be inert.
- **Dynamic (time-varying) matching and learned/neural models.** gizza blocks are pure Rust + ffmpeg;
  no ML models.
- **Stereo-image, dynamics, distortion or ambience matching.** Not an EQ operation at all.
- **Streaming-platform LUFS presets by name.** Named platform targets are trademarks; the tool takes
  a numeric `target_lufs` instead, which expresses the same thing without borrowing branding.

> Original work only — no competitor copy, branding, or trademarks reproduced.
