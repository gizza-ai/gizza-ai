# drum-pattern-generator — competitor analysis (2026-09-04)

Scope: one `WebSearch` for "online drum pattern generator MIDI download free browser", then
five reachable, real tool pages skimmed. Everything below is **paraphrased** — no competitor
copy, branding, logos or trademarks were reproduced, and no competitor assets were used.

## Competitor profiles (paraphrased)

### 1. muted.io — drum-pattern library
- **Function:** a browsable catalogue of ready-made grooves, each with a per-pattern `.mid` download.
- **Styles offered:** roughly 16 named grooves — basic rock, two funk variants, a straight EDM beat,
  classic hip-hop, jazz swing, reggae, disco, blues shuffle, bossa nova, rock shuffle, heavy metal,
  latin samba, breakbeat, downtempo/trip-hop, trap.
- **Grid:** 16 steps, one bar; three visible voices (kick, snare, hi-hat).
- **Kit:** a small numbered kit picker (1–4).
- **Params:** none exposed — the patterns are fixed. No tempo, swing, bars or time-signature control.
- **Output:** `.mid` per pattern + in-page playback.
- **Limits stated:** none.
- **Free/paid:** free.

### 2. muted.io — circular step sequencer ("lil beat maker")
- **Function:** hand-programmed 16-step loop on a circular timeline.
- **Params:** tempo (BPM), master volume in dB, per-step velocity set via number keys in 10 % steps
  (10–90 %), kit picker (5 kits, acoustic → electronic).
- **Kit pieces:** 5 voices per kit — kick, snare, closed hat, tom, and one extra percussive voice
  (clap/snap/cowbell/tom depending on kit).
- **UX:** click a step to toggle, Play / Randomize / Clear buttons, kit tabs, playhead animation.
- **Output:** `.mid` download.
- **Limits stated:** fixed 16 steps; no time-signature or swing control documented.
- **Free/paid:** free.

### 3. freesongwritingtools.com — drum beat generator
- **Function:** genre-templated 16-step grid with in-browser synthesis and MIDI export.
- **Genre templates:** basic rock, boom bap, four-on-the-floor, lo-fi hip hop, trap.
- **Params:** BPM slider; genre-typical BPM guidance in the copy (e.g. lo-fi ≈ 70–90, trap ≈ 130–170).
- **Kit pieces:** 6 voices — kick, snare, closed hat, open hat, clap, percussion.
- **UX:** one-click Randomize; Web Audio playback; a fairly long FAQ (what a beat generator is,
  how BPM works per genre, how MIDI export lands in a DAW, how the sounds are synthesized).
- **Output:** `.mid` on General-MIDI drum mapping; explicitly lists DAW compatibility.
- **Params NOT offered:** swing, complexity/density, fills, velocity/accent, bar count, time signature.
- **Free/paid:** free.

### 4. onemotion.com — drum machine
- **Function:** playable virtual kit + step composer.
- **Kit pieces:** snare, hi-hat, kick, three toms, ride, crash.
- **Params:** volume, kit choice, an FX section, pitch, and a `1:1 / 2:1 / 3:2` timing-ratio selector
  (i.e. a shuffle/subdivision control).
- **Presets:** genre banks — rock (8), blues shuffle (4), disco (2), jazz (1), waltz (1), reggae (1).
  Notably the only competitor in this set whose preset list includes a **3/4 (waltz)** groove.
- **UX:** add/delete sequences, keyboard triggering, device-motion triggering on mobile.
- **Output:** no documented file export on the page.
- **Free/paid:** free.

### 5. drum-machine.app (SEQ-16)
- **Function:** the most feature-dense of the set — 16-track step sequencer with sample library.
- **Params:** tempo 40–240 BPM, swing 0–100 %, humanize 0–50 ms of timing jitter, master volume,
  master tone lowpass, overdrive, reverb/delay wet amounts; per-track volume, pan, tune, decay.
- **Kit pieces:** 16 tracks — kicks, snares, low/mid/high toms, closed and open hats, crash, ride,
  cowbell, claves, conga, rimshot, percussion.
- **Pattern structure:** four pattern banks (A–D), pattern chaining into a song timeline, export loop
  count 1–32 repetitions.
- **Generation:** an assisted pattern generator with a 0–100 "variation" slider controlling mutation.
- **Output:** `.mid`, rendered `.wav`, per-track stem WAVs in a ZIP, one-shot slices.
- **UX:** keyboard shortcuts, drag-and-drop sample import, large bundled sample library.
- **Limits stated:** up to 16 steps per pattern; no account, no subscription.

## Table stakes distilled

| Table stake | Seen in | Our fit |
| --- | --- | --- |
| Named genre/style presets | 1, 3, 4, 5 | **built** — 18 `genre` values |
| Tempo control with a sane range | 2, 3, 5 (40–240) | **built** — `tempo`, 0 = the genre's typical BPM, else 20–300 |
| 16-step / one-bar-of-4/4 grid | 1, 2, 3, 5 | **built** — the grid is derived from `time_signature` + `hat_subdivision` |
| Multi-bar / loop repetition | 5 (1–32 export loops) | **built** — `bars` 1–64 |
| Swing / shuffle | 4 (ratio selector), 5 (0–100 %) | **built** — `swing` 0–75 % |
| Humanize (timing jitter) | 5 (0–50 ms) | **built** — `humanize` 0–100 %, deterministic per `seed` |
| Velocity / accents | 2 (per-step 10–90 %) | **built** — `velocity` base 1–127, accents and ghosts derived |
| Randomize / variation | 2 (Randomize), 3 (Randomize), 5 (variation slider) | **built** — `seed` gives reproducible variations |
| Kit choice | 1 (4), 2 (5), 5 (samples) | **built** — `kit` maps to 8 General-MIDI drum-kit programs |
| Full kit of voices, not just 3 | 4, 5 | **built** — 20 GM voices incl. toms, ride, crash, clap, cowbell, claves, congas, shaker, tambourine |
| MIDI export on GM drum mapping | 1, 2, 3, 5 | **built** — format-0 SMF, channel 10, GM note numbers |
| Audio render | 5 (WAV) | **built** — 22.05 kHz mono 16-bit WAV, rendered in Rust |
| Audible in-page playback | 1, 2, 3, 4, 5 | **built** — the WAV preview is an `<audio>` element on the page |
| Time signature other than 4/4 | 4 (a waltz preset only) | **built, and ahead** — `time_signature` covers 4/4, 3/4, 2/4, 6/8, 5/4, 7/8, 12/8 |
| Fills | none of the five | **built, and ahead** — `fill_every` writes a fill on every Nth bar |
| Density / complexity control | none of the five | **built, and ahead** — `complexity` = basic / standard / busy |
| Text/ASCII view of the pattern | none of the five | **built, and ahead** — a step grid the chat + CLI surfaces can print |

## Fit decisions

**The "rendered click preview" IS in model — we render real audio.** The brief allowed a text-only
fallback (base64 MIDI plus an ASCII grid) if audio turned out not to fit the pure-Rust page surface.
It does fit: the core synthesises the pattern to 22.05 kHz mono 16-bit PCM and hand-writes a RIFF/WAVE
header, with no crate beyond `midly` and no ffmpeg. Voices are deterministic DSP — decaying pitch-swept
sines for kick and toms, filtered noise plus a body tone for snare/claps, short high-passed noise for
hats and cymbals — and the noise source is a seeded xorshift PRNG, so the same parameters always
produce byte-identical audio. The page renders that WAV in an `<audio controls>` element next to the
MIDI download, so the page delivers **both** deliverables from the description. We keep the ASCII step
grid too, because chat and CLI are text surfaces where an `<audio>` element does not exist — there the
grid *is* the preview, and `output = preview-wav` swaps the downloadable artifact to the WAV when a CLI
user actually wants the audio.

**Preview length is capped at 30 s of audio.** Sixty-four bars of a 60 BPM 12/8 groove would be several
minutes of PCM inside a base64 data URL. The MIDI file always covers the full requested length; the
preview renders as many whole bars as fit in 30 s and says so in the summary. That is stated on the page,
not just in an error.

**Metronome click is a `preview` mode, not a separate tool.** `preview` = `drums` / `drums-and-click` /
`click` / `off`. A bare `click` render is a genuine metronome for the chosen tempo and time signature,
which is what "click preview" most literally means; `drums-and-click` is the practise-along mix.

## In-model gaps closed in this build

- Genre presets with genre-typical default tempos (the freesongwriting BPM-per-genre guidance is a
  copy feature there; here it is a real default the `tempo = 0` path reads).
- Swing and humanize as first-class params (SEQ-16 parity), made deterministic by `seed` so the same
  URL always reproduces the same file — none of the five offer reproducible randomisation.
- Accent/ghost velocity shaping derived from one `velocity` base, rather than requiring per-step edits.
- Full GM voice set and GM kit programs so the exported file sounds right in any DAW.
- Deep-linkable parameters: every control is a URL query param, so a pattern is shareable as a link.

## Out of model — considered, not built

- **Interactive click-to-edit step grid.** gizza pages are stateless input → output forms; a persistent
  editable sequencer needs its own stateful UI. Our answer is parameterised generation plus a read-only
  ASCII grid.
- **Sample library / sample import and custom kits** (SEQ-16). Needs a hosted asset library; the
  browser-local model has no server to serve tens of thousands of samples.
- **Mixer FX — reverb, delay, overdrive, per-track pan/tune/decay** (SEQ-16). Buildable in principle but
  it would triple the parameter count for a *preview* renderer; the MIDI file is the artifact a user
  mixes in their DAW.
- **Stem WAVs as a ZIP, song mode / pattern chaining, one-shot slicing** (SEQ-16). Multi-artifact output
  does not fit the single-artifact response envelope.
- **ML / LSTM "AI" pattern generation** (seen on adjacent tools in the search results). Needs a model
  download; gizza blocks are pure Rust/WASM.
- **Live keyboard or motion triggering** (onemotion). Real-time performance input, not a generator.
- **Accounts, cloud project saving, paid tiers.** No server, by design.
