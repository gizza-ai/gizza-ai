# chorus — competitor analysis (2026-07-30)

New tool: **chorus** — "Thickens a sound with short modulated delay voices for a
chorus effect." ffmpeg audio-input family (`Input::Audio`), single in / single
out, built on the stock ffmpeg `chorus` filter
(`chorus=in_gain:out_gain:delays:decays:speeds:depths`, one `|`-separated entry
per voice).

## Not a duplicate

`audio-effects-rack` already offers a **chorus** *stage*, but only as a two-value
preset (`none|light|deep`) inside a five-effect chain with no fine control. The
repo pattern is explicit: dedicated single-effect tools (`audio-eq`,
`audio-fade`, `audio-compress`, `audio-pitch-shift`) coexist with the rack. A
standalone `chorus` tool exposing the real per-voice controls (voices, delay,
depth, rate, decay) is the dedicated counterpart, not a redundant tool.

## Competitors scanned

1. **ElysiaTools — Audio Chorus** (elysiatools.com/en/tools/audio-chorus) — the
   closest analogue: also an ffmpeg `chorus` wrapper. Exposes the raw filter
   knobs **per voice** — `inGain 0.5`, `outGain 0.9`, `delay 40–50 ms`,
   `decay 0.4`, `speed 0.25–0.3 Hz`, `depth 2` — but through a **raw JSON-array
   textarea** ("Voices" field). Output formats: mp3, aac, m4a, ogg, opus, flac,
   wav. 100–200 MB cap. Default config = two voices. **UX weakness: hand-edited
   JSON, no sliders, no presets.**
2. **AudioToolset — Chorus** (audiotoolset.com/chorus) — upload → fullscreen
   editor → download. No exposed numeric parameters or documented ranges; a
   generic "editor" with no chorus-specific controls surfaced.
3. **AudioKit — Chorus** (audiokit.in/tools/chorus) — "make sound richer and
   thicker instantly." Marketing-only page; no documented adjustable parameters,
   ranges, or presets — effectively a one-click effect.

(Melobytes.com/en/app/chorus_effect returned HTTP 403 and was replaced by
AudioKit to keep three real competitors.)

## Table-stakes parameters (from DSP references: Unison, iZotope, Native
Instruments) and fit-to-model tags

| Capability | In our tool | Fit |
|---|---|---|
| **Rate** (modulation speed, Hz) | `speed_hz` (0.1–5, default 0.4) | in-model → `speeds` |
| **Depth** (modulation depth) | `depth_ms` (1–8 ms, default 2) | in-model → `depths` |
| **Delay** (base delay, ms) | `delay_ms` (20–80, default 50) | in-model → `delays` |
| **Voices** (number of copies) | `voices` (2–4, default 2) | in-model → voice count, spread deterministically |
| **Amount / effect level** | `decay` (0.1–0.9, default 0.4) | in-model → `decays` (per-voice level ≈ wetness) |
| **Output format** | `format` mp3/wav/ogg/flac/m4a | in-model |
| Dry/Wet **Mix** knob | — | **out-of-model**: ffmpeg's `chorus` has no separate dry/wet control; the dry signal is fixed at `in_gain` and per-voice level via `decays` (`decay` param) is the closest analogue. Not forced in. |
| **Feedback** (repeats fed back) | — | **out-of-model**: the ffmpeg `chorus` filter has no feedback path (that's `aecho`/flanger territory). Listed, not built. |
| Opus / AAC extra output formats | — | out-of-model here: family-standard output set is mp3/wav/ogg/flac/m4a (matches every other audio tool); not expanded for one tool. |

Every table-stake lands in the descriptor or is explicitly listed out-of-model
above — none dropped silently.

## Our differentiators (in-model, built)

- **Friendly sliders** for every numeric control (voices/delay/depth/rate/decay)
  instead of ElysiaTools' raw JSON textarea.
- **Preset chips** (`[[example]]`): Subtle Double, Classic Chorus, Lush Ensemble,
  Fast Shimmer — one-click starting points competitors lack.
- Deterministic per-voice spread (delay staggered +8 ms/voice, rate scaled per
  voice) so extra voices sound genuinely distinct instead of phase-cancelling.
- Fully browser-local (no upload), consistent with the rest of the audio family.

No competitor copy, branding, or trademarks were copied — analysis is paraphrased.
