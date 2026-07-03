# audio-pitch-shift — competitor analysis (2026-07-03)

Two passes, same day:

1. **Build-time scan** (kept below) — one WebSearch, 3-6 tools skimmed, table-stakes
   decisions for the initial build.
2. **Improve-pass deep dive** (this section) — top 5 real competitor tools, one
   read-only researcher each, full params/defaults/formats/UX/SEO profiles.
   All paraphrased; no competitor copy, branding, or assets reused.

## Improve pass — the 5 profiles (paraphrased)

### 1. SoundTools pitch shifter (soundtools.io/pitch-shifter)
- **Params:** semitone slider, −12…+12 (whole steps); preset buttons ±1 semitone +
  a "custom" mode. No cents control, no tempo toggle (a separate speed tool exists).
- **Formats:** in mp3/wav/flac/aac/ogg; out = same container as input, no picker,
  no bitrate options.
- **UX:** live re-adjustable preview while playing, spacebar shortcut, plain
  file-picker (no drag-drop), no waveform, no reset.
- **SEO:** targets both "pitch shifter" and "pitch changer"; music-theory education
  (semitone = adjacent piano keys); worked key transpositions (C major +2 → D major,
  G major −3 → E major); karaoke (−2…−3 for vocal range), guitar drop-tuning
  emulation, harmony layering (±3/4/5/7); FAQ on why big shifts sound robotic.
- **Notes:** processing location ambiguous ("in your browser" copy, no
  files-never-leave claim); no stated size limits.

### 2. Audioalter pitch shifter (audioalter.com/pitch-shifter)
- **Params:** single slider −24…+24 semitones (widest range seen); fractional values
  accepted by the backend (their 440→432 Hz preset is −0.3177 st); no number box.
- **Formats:** in mp3/wav/flac/ogg; no output format/quality options documented.
- **UX:** drag-drop upload, submit-then-wait job, before/after audio samples embedded
  as worked examples; no preview of the user's own file, no waveform, no reset, no FAQ.
- **SEO:** "12 semitones = 1 octave" framing; a dedicated 432 Hz-converter landing
  page as a preset variant; numbered how-to steps.
- **Notes:** server-side (upload + 50 MB cap) — our browser-local story is a
  differentiator.

### 3. vocalremover.org pitcher (vocalremover.org/pitch)
- **Params:** pitch slider −6.00…+6.00 st at 0.01-st (1-cent) resolution, with a
  "snap to semitones" toggle (off by default); independent speed slider 0.5×–1.5×.
- **Formats:** in ~any audio; out mp3 (default) or wav; no bitrate options.
- **UX:** auto-detects key/scale/BPM on load; live target-key readout as the slider
  moves; waveform player with seek/loop/level meters; whole-page drag-drop; output
  auto-named with resulting key + bpm; confirm dialog before discarding a track;
  no reset button, no FAQ, minimal page copy.
- **SEO:** transpose-music language, key/BPM detection headline, serverless/privacy
  claim ("files never leave your device").
- **Notes:** genuinely browser-local; narrow ±6 range; behind Cloudflare (researched
  by driving the app in a browser).

### 4. pitchchanger.io (pitchchanger.io/audio-pitch-changer)
- **Params:** pitch slider −12…+12 st, step 1, signed live readout; speed slider
  0.5×–1.5× step 0.05; process button disabled at neutral settings with a hint.
- **Formats:** in mp3/wav/flac/m4a/aac (250 MB cap shown inline); out WAV (framed
  as the high-quality choice) or MP3; no bitrate knobs.
- **UX:** drag-drop zone with format badges; waveform strip with click-to-seek;
  file-info cards (duration original vs current, BPM during playback, key detection
  "coming soon"); progress % + success banner; "upload different file" instead of
  reset; separate long-form SEO article page cross-linking the tool; ~24 locales.
- **SEO:** stacks "pitch changer / song key changer / transpose music online";
  worked examples (+2 = C→D, −3 = C→A, +12 = octave); quality guidance (stay within
  ~6 st overall, ~4 for vocals); phase-vocoder/granular explainer; a
  history-of-pitch-shifting timeline as authority content; persona list (singers,
  karaoke, theater auditions, guitarists avoiding capos, Bb/Eb horn players, ear
  training, DJ harmonic mixing, teachers, transcribers).
- **Notes:** ffmpeg-wasm + SoundTouch, genuinely local (verified via network log);
  marketing says phase vocoder while telemetry says SoundTouch (WSOLA).

### 5. OnlineToneGenerator pitch shifter (onlinetonegenerator.com/pitch-shifter.html)
- **Params:** slider + paired number box (semitones); "maintain tempo" checkbox
  (default on — off couples pitch to speed); "save output" checkbox that records
  playback to an mp3.
- **Formats:** in mp3/wav only; out mp3 (real-time capture — must play the whole
  file to record it).
- **UX:** live playback preview; percent-of-original readout; recordings list;
  no waveform, no presets, no drag-drop, no reset.
- **SEO:** singer backing-track transposition; guitarist angle (shift a half-step-
  down recording up 1 st to play along in standard tuning); "one semitone ≈ 5.946%
  frequency change" educational hook.
- **Notes:** Web Audio, fully local; the record-in-real-time download path is weak —
  our offline ffmpeg render is strictly better.

## Gap list (ours vs the 5) and decisions

| # | Gap (≥1 competitor has it) | Dimension | Fit | Decision |
|---|---|---|---|---|
| 1 | Slider control for the shift (all 5 use one) | UX | in-model | **Built** — declarative `kind = "slider"` in the shared generator (range + number pair, one run per drag-release); no slug branches |
| 2 | One-click presets (SoundTools ±1; Audioalter 432 Hz page) | UX | in-model | **Built** — 5 `[[example]]` chips: ±12, ±1, 440→432 Hz (−0.32) |
| 3 | Fractional input without browser step-mismatch nags | UX | in-model | **Built** — number-typed params now render `step="any"` (platform-wide) |
| 4 | Copy-paste-runnable CLI example (our page card omitted required `semitones`) | copy | in-model | **Built** — page + markdown CLI examples now include schema-derived samples for every field (platform-wide) |
| 5 | Key-transposition worked examples, karaoke/guitar/harmony personas (SoundTools, pitchchanger, OTG) | copy/SEO | in-model | **Built** — semitone cheat-sheet section, E♭-tuning FAQ + example, harmony intervals, "pitch changer" synonym FAQ, ~5.95%/semitone fact |
| 6 | "Pitch changer" keyword targeting (SoundTools, pitchchanger) | copy/SEO | in-model | **Built** — meta description/tags + FAQ equivalence entry |
| 7 | Live preview while dragging (SoundTools, vocalremover, OTG, pitchchanger) | capability | out-of-model | page framework is run-per-change (no live audio graph); slider commits one run per release |
| 8 | Key/scale/BPM detection + target-key readout (vocalremover; pitchchanger "soon") | capability | out-of-model | needs audio analysis outside the argv-builder model; FAQ teaches counting semitones |
| 9 | Formant-preserving voice mode (pro shifters) | capability | out-of-model | no librubberband in either ffmpeg build; FAQ states the ±4 st natural range honestly |
| 10 | Combined speed+pitch controls on one page (vocalremover, pitchchanger, OTG toggle) | capability | out-of-model here | deliberate split — change-speed owns tempo; page copy cross-links |
| 11 | Bigger input caps (50–250 MB server tools) | capability | out-of-model here | 10 MiB is the family-wide envelope cap; revisit family-wide, not per-tool |
| 12 | Output auto-named with detected key/bpm (vocalremover) | UX | out-of-model | depends on gap 8; family keeps the deterministic `-pitch-shifted.<ext>` suffix |

Engine capabilities (range ±24, 1-cent precision, tempo-exact chain, 5 output
formats, any-decodable input) were already at or ahead of best-in-class — no
core/descriptor change; the chat schema is unchanged in this pass.

---

# Build-time scan (original, kept for the record)

One WebSearch ("online pitch shifter change audio pitch semitones without changing tempo
tool"); skimmed the top real tools: Audioalter pitch-shifter, SoundTools pitch-shifter,
vocalremover.org pitch, Tembrica pitch, AudioSpeedChanger, x-minus.pro transpose.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Shift by semitones, up or down | all of them | in-model | required `semitones` number param |
| Tempo/duration preserved | all (the tool's definition); Tembrica exposes it as a "lock tempo" toggle | in-model | always on — asetrate+aresample+atempo chain; change-speed is the sibling for the speed case |
| ±24 semitone range | Audioalter (two octaves); most others ±12 | in-model | ±24, atempo chained per instance beyond ±12 |
| Fine/fractional shifts (cents) | vocalremover.org (fine slider) | in-model | `semitones` is f64 — 0.5 = 50 cents, 0.01 = 1 cent |
| Common output formats | SoundTools (mp3/flac/wav/aac/ogg) | in-model | family-standard `format` enum mp3/wav/ogg/flac/m4a, default mp3 |
| Real-time preview while dragging | vocalremover.org, Tembrica | out-of-model | page framework is run-per-change, no live audio graph |
| Key detection / suggest target key | vocalremover.org | out-of-model | needs pitch detection (ML-ish); FAQ teaches counting semitones instead |
| Formant-preserving voice mode | (pro/studio shifters) | out-of-model | no rubberband in either ffmpeg build; FAQ states the ±4-semitone natural range honestly |
| Combined speed+pitch control | AudioSpeedChanger, x-minus | out-of-model here | deliberate split: change-speed owns tempo; page copy cross-links the distinction |

## Design decisions

- Stock-ffmpeg chain (`aresample=44100,asetrate=<rate>,aresample=44100,atempo=<44100/rate>`)
  — librubberband is absent from both the native runtime and @ffmpeg/core, so the resample
  trick is the only in-model implementation. atempo is computed from the ROUNDED asetrate so
  duration is exact; residual pitch error < 0.04 cents.
- atempo chained (0.5..2 per instance) so the full ±24 range works on conservative builds;
  a unit test pins the +24/-24 chains.
- semitones=0 (the empty page field) rejected with a guiding error that names both
  directions and points format-only users at audio-convert.
- Family invariants kept: shared Format enum + 192k lossy codec args, `-vn`, 10 MiB caps,
  `-pitch-shifted.<ext>` filename suffix, drift-guard schema test.

## Verification (all run, all green)

- Unit: 12 core + 2 block tests (argv exactness, chain bounds, rounding, errors, drift guard).
- CLI vs the public 1.26 s beep: +12 → window zero-crossing freq 2110→4075 Hz (×1.93 on a
  harmonic-rich beep), duration 1.254→1.201 s (preserved, not halved); -12 → 1057 Hz vs
  ~1055 Hz expected, duration 1.260 s exact. Exact-text no-op and range errors exercised.
- Playwright on the 440 Hz sine fixture: +12 → measured ~880 Hz still ~3 s; deep link
  `?semitones=-12&format=wav` → ~220 Hz still ~3 s; bare upload → guiding no-op error.
  Bounds pre-measured with local ffmpeg (880.0 / 220.0 Hz on the same chain).

## Improve-pass verification (2026-07-03, all run, all green)

- Phase 1 baseline: 14 unit tests (incl. drift guard) green; CLI +12/0/30 against the
  public beep (duration 1.254 s preserved; both guiding errors exact); Playwright 3/3.
- Post-improve: generator 43 unit tests (slider/step-any/CLI-example cases added);
  Playwright 5/5 incl. new slider-commit (+12 → ~880 Hz) and preset-chip-after-error
  (−12 → ~220 Hz) cases; sibling regression suites (trim-audio, audio-eq, calculator,
  age-calculator) 9/9; per-slug hygiene gate exit 0; `wafer build` OK (548.5 KiB);
  CLI smoke of the page's copy-paste example verbatim.
