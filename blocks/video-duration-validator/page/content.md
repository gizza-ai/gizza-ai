## About this tool

Most duration checkers just tell you how long a clip is and leave the arithmetic to
you. This one takes the spec as well: give it the length the video is *supposed* to
be, and it answers with a flat **PASS** or **FAIL** plus the exact delta in seconds,
so you can drop it into a delivery checklist or a build script without eyeballing a
timecode.

The duration comes straight from the container header — the `moov` atom in an
MP4/MOV, the Segment Info in a Matroska/WebM, the fmt/data chunks in a WAV. Nothing
is decoded and nothing is re-encoded, so a two-hour master answers as fast as a
six-second bumper. It all runs in WebAssembly inside this page: the file never
leaves your device, there is no upload and no account.

### A worked example

Pick a clip that is really 31.25 seconds long, set **Target length** to `30`, leave
**Tolerance** at `0.5` and **Comparison** at `within`. The report reads:

```json
{
  "status": "FAIL",
  "pass": false,
  "reason": "too_long",
  "mode": "within",
  "actual_seconds": 31.25,
  "actual_duration": "0:31.250",
  "target_seconds": 30.0,
  "target_duration": "0:30.000",
  "tolerance_seconds": 0.5,
  "delta_seconds": 1.25,
  "overshoot_seconds": 0.75,
  "allowed_min_seconds": 29.5,
  "allowed_max_seconds": 30.5,
  "container": "MP4 / M4A (ISO BMFF)",
  "summary": "FAIL — 0:31.250 (31.25s) is 0.75s too long for the rule 30s ± 0.5s."
}
```

Read it as: the clip is `delta_seconds` = 1.25 s longer than the 30 s target, and
because the allowed window stops at 30.5 s it is `overshoot_seconds` = 0.75 s past
the limit. Trim 0.75 s and it passes. Had the clip been 30.2 s, `status` would be
`PASS` and `overshoot_seconds` would be `0`.

### Reading the controls

- **Target length (seconds)** — the length the clip is supposed to be. Decimals are
  fine (`29.97`), and the range is 0.001 s to 24 hours.
- **Tolerance (± seconds)** — how much slack the target gets. `0.5` is a sensible
  default: it absorbs the fraction of a frame that encoders round away, so a clip
  cut to exactly 30 s at 29.97 fps still passes. Set `0` when the spec is literal.
- **Comparison** — `within` checks both ends (`target ± tolerance`); `max` only
  checks the upper end, for "must not exceed" limits like an ad slot or a
  short-form cap; `min` only checks the lower end, for "must be at least" rules
  like a minimum runtime.

Use the **Try** chips above the form as starting points for the common specs — a 6 s
bumper, a 15 s or 30 s spot, a 60 s or 180 s short-form cap, a ten-minute minimum.
They are just presets for the two numbers, so edit them freely: platform limits
change, and the tool checks whatever number you give it rather than a list baked in
last year.

### Limits worth knowing

- Containers understood: MP4/M4A/MOV, Matroska/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3
  and AAC/ADTS. Audio files work exactly the same way as video.
- Target and tolerance both accept 0 – 86 400 s (24 hours); the target must be
  greater than zero.
- The duration reported is the **container's** duration, which is what players,
  ad servers and delivery portals read. It is not a frame-accurate decode count, so
  a file whose header disagrees with its packets will be judged on the header.
- A file that records **no** duration at all — a screen-capture WebM from
  `MediaRecorder`, an MP4 with a damaged `moov` atom — is reported as an error, not
  as a FAIL. Repair it first, then re-check.
- One file at a time; there is no batch mode on this page.

## FAQ

<details>
<summary>What exactly is the difference between delta_seconds and overshoot_seconds?</summary>

`delta_seconds` is simply `actual - target`: how far the clip is from the target,
positive when it is too long and negative when it is too short. `overshoot_seconds`
is how far it landed **outside the allowed window**, and it is `0` on every PASS.
With a 30 s target and 0.5 s tolerance, a 31.25 s clip has a delta of 1.25 but an
overshoot of only 0.75 — that 0.75 s is the amount you actually have to trim.

</details>

<details>
<summary>Which mode should I use — within, max or min?</summary>

Use `within` when the clip has to hit a length: a 30-second spot, a 6-second
bumper, a fixed-length loop. Use `max` when there is only a ceiling — "must not
exceed 60 seconds" — so a short clip still passes. Use `min` when there is only a
floor, such as a minimum runtime for a course module or a minimum watch time. In
`max` and `min` the tolerance is still applied, in the forgiving direction, so
`max` with a 60 s target and 0.5 s tolerance really passes anything up to 60.5 s.

</details>

<details>
<summary>Why did my clip fail by a few milliseconds?</summary>

Encoders round the last frame. At 29.97 fps a single frame is about 33 ms, so a
timeline cut at exactly 30.000 s often lands at 30.033 s or 29.967 s in the
container. That is why the default tolerance is 0.5 s rather than 0. If you need a
literal match, set the tolerance to `0` and expect to see those millisecond
differences — they are real, and delivery portals that reject on them are checking
the same number this tool reads.

</details>

<details>
<summary>It says no duration is recorded. What now?</summary>

The container header is missing or broken — the classic case is a WebM written by a
browser's `MediaRecorder` or a screen recorder that was killed mid-write, whose
duration reads as `Infinity` in players. There is nothing to compare against, so
this is an error rather than a FAIL. Remux the file with stream copy to rebuild the
header (the duration-fix remux tool does exactly that, losslessly), then run the
check again.

</details>

<details>
<summary>Does it work on audio files too, and is anything uploaded?</summary>

Yes to audio — WAV, MP3, FLAC, OGG, AIFF, CAF and M4A are all read the same way, so
you can validate a 30-second radio spot or a podcast minimum length with the same
rule. And nothing is uploaded: the parser is compiled to WebAssembly and runs in
this tab. Load the page, go offline, and the check still works.

</details>
