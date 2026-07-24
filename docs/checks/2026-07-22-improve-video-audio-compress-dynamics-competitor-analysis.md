# Dynamic-Range Audio Compression for Video — Competitor Analysis (2026-07-22)

Tool: `video-audio-compress-dynamics` — applies dynamic-range compression to a
video's audio (evens out loud/quiet passages) via ffmpeg `acompressor`, picture
stream-copied, audio re-encoded. Presets (light/medium/heavy) + makeup-gain
toggle.

## Competitor comparison

| Tool | How it's framed | Manual DRC knobs? | Presets / one-click | Underlying approach | Multi-file / batch | Video in/out |
|------|-----------------|-------------------|---------------------|---------------------|--------------------|--------------|
| **Auphonic** | "Adaptive Leveler" + loudness targets | Semi: Dynamic Range (LU), compressor Soft/Medium/Hard/Off, scope | Leveler presets (Default, Foreground Only, Fast, Amplify Everything) | ML/statistical adaptive leveling + micro-dynamics compression + loudness norm | Yes (cloud batch) | Yes |
| **Adobe Podcast (Enhance / Express)** | "Enhance speech" one-click | No | Single strength slider | ML speech re-synthesis (bundles compression + denoise + EQ) | Limited | Audio-centric; video via Express |
| **Descript (Studio Sound + Advanced Audio Mixing)** | Transcript editor with audio cleanup | Partial: Studio Sound intensity slider; Advanced Mixing exposes leveling/compression | Studio Sound one-click; loudness targets | ML enhancement + leveling/normalization | Project-based | Yes |
| **Kapwing** | "Auto Level Volume" / audio enhance | Mostly no (slider boost) | One-click Auto Level; volume slider | Loudness analysis + auto gain/leveling | Per-project | Yes |
| **VEED / Clideo** | "Clean audio" / volume + enhance | No (toggle/slider) | One-click enhance, volume slider | Server-side enhance (denoise + level) | Single file typical | Yes |

## Table-stakes features (nearly all offer)

- **One-click "make it consistent"** as the primary CTA; manual DRC knobs hidden or absent for consumer tools.
- **A target/intensity control** — strength slider (Adobe, Descript, Kapwing) or loudness/dynamic-range target in LU/LUFS (Auphonic, Descript).
- **Makeup / auto-gain handled automatically** so output isn't quieter after compression.
- **Broad format support**, direct video export with processed audio muxed back in.
- **Sensible output naming** (source name + suffix like `-enhanced`/`-leveled`).
- **Denoise + leveling bundled** in the "enhance" action.

## Params / defaults / UX per tool

- **Auphonic** — "Dynamic Range" in dB/LU + compressor selector Auto/Soft/Medium(default)/Hard/Off + scope. Framed as targets, not threshold/ratio.
- **Adobe Podcast** — single 0–100% "Enhance Speech" strength; no threshold/ratio; speech-only.
- **Descript** — Studio Sound intensity slider + Advanced Audio Mixing with leveling/compression amounts + loudness targets.
- **Kapwing** — "Auto Level Volume" + clip-safe volume Booster slider; no ratio/attack/release.
- **VEED / Clideo** — one-click clean/enhance + volume slider; no DRC params exposed.

Non-expert framing pattern across all: sell the *outcome* ("consistent", "same
room", "no more loud/quiet"), expose at most a single strength control.

## In-model vs out-of-model

`acompressor` defaults for reference: threshold 0.125 (≈ −18 dBFS), ratio 2,
attack 20 ms, release 250 ms, makeup 1, knee 2.83.

**IN-MODEL (shipped):**
- Light / medium / heavy presets → threshold/ratio/attack/release tuples
  (light ratio 2 / −18 dB, medium ratio 4 / −24 dB, heavy ratio 8 / −30 dB,
  faster attack as it gets heavier). Mirrors the proven presets already in
  `audio-effects-rack` so behaviour is consistent across the toolkit.
- **Makeup-gain toggle** (`makeup`) so heavier compression doesn't lose
  perceived loudness — the single most valuable exposed knob, matches
  competitors' "auto-gain". On by default; off pins gain to unity for
  peak-taming without a level boost.
- Outcome-first framing ("even out loud and quiet parts", preset names instead
  of raw DSP terms) — matches consumer UX.
- Single-video upload, video track stream-copied (fast, lossless), audio-only
  re-encode, `-dynamics` output naming, container preserved.

**OUT-OF-MODEL (not built):**
- Adaptive/segment-aware leveling distinguishing speech vs music (Auphonic/Descript) — needs content analysis.
- Loudness-target normalization to a specific LUFS with a "comfort zone" — needs a `loudnorm` two-pass analysis (a different, separate capability from DRC; kept out to keep this tool focused).
- ML speech enhancement / re-synthesis / denoise (Adobe Enhance, Studio Sound).
- Batch / multi-file, cloud project workflows, transcript-linked editing.

**Decision:** ship 3 presets + makeup toggle as the core — covers the
table-stakes "one-click make audio consistent". Loudness targeting (`loudnorm`)
is intentionally kept out as a distinct analysis-pass feature, not folded into
these DRC presets.
