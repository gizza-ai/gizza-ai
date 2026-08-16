## About this tool

A single "the track is 128 BPM" number describes a click track, not a performance. Real playing
breathes: it pushes into a chorus, drags in a ballad, and lands a deliberate ritardando at the
end. A **tempo map** captures that — the tempo at every beat, plotted against time — and it is
what a DAW needs before a grid, a click, or quantised MIDI will line up with a live take.

This tool builds that map from the one artefact every beat tracker, DAW and tap session already
gives you: **a list of beat times**. Paste them in and each consecutive pair becomes one
instantaneous tempo reading, so *N* beats produce *N−1* readings — the BPM-versus-time curve.

It reads the exports you already have. Each line's first field is the time, so an Audacity label
track (`start`, `end`, `label`), a CSV column, a marker export or a bare column of numbers all
work as-is; blank lines, a header row and `#` or `//` comments are skipped. Times can be decimal
seconds (`1.75`), unit-suffixed (`1750ms`), `m:ss.mmm` (`0:01.750`), `h:mm:ss.mmm`, or
`hh:mm:ss:ff` frame timecode — set the frame rate for the last one. A single comma-separated line
works too.

### Worked example

Five beats from a take that gradually slows down:

```
0.000
0.500
1.020
1.580
2.200
```

With the defaults (quarter-note beats, no smoothing, CSV out) the tempo map is:

```
time_seconds,bpm,beat,interval_ms
0.000,120.00,1,500.0
0.500,115.38,2,520.0
1.020,107.14,3,560.0
1.580,96.77,4,620.0
```

Four readings from five beats, dropping from 120 BPM to just under 97 — a clear ritardando.
Switch **Export as** to *Summary* and the same input reports the mean, median, range, drift,
standard deviation, interval jitter, the overall average across the take, the trend in BPM per
minute, a stability rating and the conventional tempo marking. Switch it to *MIDI tempo map* and
you get `tick,microseconds_per_quarter,bpm` rows — one event per tempo change, placed at your
chosen ticks-per-quarter-note division.

### Cleaning up hand-tapped input

Tapping is noisy, and raw beat-to-beat readings expose every millisecond of it. Three controls
fix that without hiding real tempo movement:

- **Smoothing window** applies a centred moving average across *N* beats. Try 4–8 for tapping;
  leave it at 1 to see every raw reading.
- **Smooth with → Median** takes the middle value of the window instead of the average, so one
  badly-placed beat is ignored rather than dragged through the curve.
- **Drop beats closer than** removes accidental double taps and duplicated markers; 80–200 ms is
  a sensible guard.

### Half-time, double-time and compound pulses

If the curve reads 65 BPM when the music is clearly at 130, you marked every other beat. Set
**Each marked beat is a** to *Half note* and the reading doubles; choose *Eighth note* if you
marked twice per beat. Dotted-quarter handles a compound 6/8 pulse, and triplet-eighth handles
three marks per beat. Whatever you pick, the result is reported in standard quarter-note BPM.

### Plotting the curve

Per-beat rows are unevenly spaced in time, which some plotting tools dislike. Set an **even time
grid** — say 1 second — and the curve is resampled onto a regular axis, each row holding the
tempo of the beat interval it falls inside. The CSV's first two columns are `time_seconds` and
`bpm`, so it drops straight into a spreadsheet chart or any plotting library.

### Limits and edge cases

- At least **2** beat times are required (one timestamp has no interval), and at most
  **20,000** per run.
- Times must **increase** after the offset and the double-tap filter is applied; the tool reports
  the offending beat rather than silently sorting your data.
- An exact duplicate time is an error unless *Drop beats closer than* is above 0.
- An even time grid can't be combined with the MIDI export, because MIDI tempo events have to
  land on real beats.
- Statistics are always computed from the per-beat curve, so they stay the same whether or not
  you resample onto a grid.
- Frame rate applies only to `hh:mm:ss:ff` timecode; every other format ignores it.
- Everything runs locally in your browser — the beat times are never uploaded.

## FAQ

<details>
<summary>Can I feed it an audio file and have it find the beats?</summary>

No — this tool starts from beat *times*, not audio. Getting those times out of a recording is a
separate job: a beat tracker, your DAW's beat-detection or transient-marker feature, or simply
tapping along and recording the timestamps. Once you have the list, paste it here and you get
the tempo curve, the statistics and the exports.

</details>

<details>
<summary>Why do 100 beats give only 99 BPM readings?</summary>

Tempo is measured *between* beats, not at them: one reading needs two timestamps. So a list of
*N* beats yields *N−1* intervals and *N−1* tempo readings. Each row is timed at the beat the
interval starts on, which is why the last beat has no row of its own.

</details>

<details>
<summary>My tempo map jumps wildly between beats. Is the input wrong?</summary>

Probably not — raw beat-to-beat readings are extremely sensitive. A 10 ms tapping error on a
500 ms beat is already a 2.4 BPM swing, and one missed beat halves the reading for that interval.
Raise the smoothing window to 4–8 beats, switch the method to *Median* so a single stray beat is
ignored, and set *Drop beats closer than* to about 120 ms to remove double taps. The summary's
interval jitter figure tells you how noisy the input was in milliseconds.

</details>

<details>
<summary>What exactly does the MIDI tempo map export contain?</summary>

One row per tempo change, as `tick,microseconds_per_quarter,bpm`. The tick column places each
event on its beat using your ticks-per-quarter-note division (480 and 960 are the usual values,
so match the file or project you are importing into), and microseconds per quarter note is the
value a Standard MIDI File tempo meta event actually stores — 500,000 is 120 BPM. Consecutive
rows with the same rounded BPM are collapsed, because a tempo map only needs an event where the
tempo actually changes.

</details>

<details>
<summary>Which time formats can I paste?</summary>

Decimal seconds (`1.75`), a value with a unit (`1750ms`, `1.75s`), `m:ss.mmm` (`0:01.750`),
`h:mm:ss.mmm` (`1:02:03.500`), and `hh:mm:ss:ff` frame timecode — set the frame rate for that
last one so frames convert correctly. A bare number is read as seconds unless you switch *A plain
number means* to *Milliseconds*. Extra columns after the time are ignored, so pasting a label
track or a whole CSV row works without editing.

</details>

<details>
<summary>How is the stability rating decided?</summary>

It comes from the drift — the gap between the fastest and slowest reading on the smoothed curve.
Under 1 BPM is reported as rock steady, under 3 as steady, under 8 as slight drift, under 20 as
variable, and anything wider as highly variable. It is a plain-language shorthand for the drift
figure printed next to it, so trust the numbers when you need precision, and remember that heavy
smoothing narrows the drift by design.

</details>
