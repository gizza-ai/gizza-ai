## About this tool

Spectral EQ matching answers one question: *how far is my mix from the tonal balance I want, and what EQ move closes the gap?* This tool does that arithmetic. You paste the per-band levels your spectrum analyser, RTA or meter already reported for your track, then either paste a reference track's band levels or pick a built-in target curve. It returns the per-band corrective gains, a ready-to-paste ffmpeg `equalizer` or `firequalizer` filter chain, and — if you supply both integrated-loudness figures — the broadband offset in dB.

No audio is uploaded or decoded here: the tool consumes numbers you already measured, so it runs instantly and offline once the page has loaded. The rendering step stays on your machine, which is exactly what the generated ffmpeg command is for.

The controls mirror what a match-EQ workflow actually needs. **Amount** scales the derived move (50 % is usually the safer starting point on music). **Maximum boost/cut** clamps every band so one badly measured frequency can't ask for an extreme filter. **Smoothing** averages neighbouring bands, trading narrow-resonance accuracy for broad tonal balance. **Band Q** sets the bandwidth of each peaking stage. **Tone only** removes the average level difference between the two curves before matching, so the EQ corrects tone and the loudness offset handles level.

### Worked example

A track measured at five octave-spaced bands, matched against a reference measured at the same bands, with ±1 band of smoothing and a loudness move from −9.5 LUFS to −14 LUFS:

```
Track:      63 -12   250 -8   1k -6   4k -10   12k -18
Reference:  63 -10   250 -8   1k -6   4k -8    12k -14
```

Output:

```
Spectral EQ match: 5 bands
Target curve: pasted reference (5 bands, interpolated onto the track bands)
Settings: amount 100% | smoothing +/-1 band(s) | limit +/-6.00 dB | Q 1 | tone-only on
Broadband level removed before matching: +1.60 dB

 Freq (Hz)     Track    Target      Diff   Smoothed      Gain
        63    -12.00    -10.00     +0.40      -0.60     -0.60
       250     -8.00     -8.00     -1.60      -0.93     -0.93
      1000     -6.00     -6.00     -1.60      -0.93     -0.93
      4000    -10.00     -8.00     +0.40      +0.40     +0.40
     12000    -18.00    -14.00     +2.40      +1.40     +1.40

Loudness: -9.5 LUFS -> -14.0 LUFS = -4.50 dB offset

ffmpeg -i input.wav -af "equalizer=f=63:t=q:w=1:g=-0.60,equalizer=f=250:t=q:w=1:g=-0.93,equalizer=f=1000:t=q:w=1:g=-0.93,equalizer=f=4000:t=q:w=1:g=0.40,equalizer=f=12000:t=q:w=1:g=1.40,volume=-4.50dB" output.wav
```

Read the table left to right: **Diff** is the raw target-minus-track difference after the broadband level was removed, **Smoothed** is that difference averaged with its neighbours, and **Gain** is the final value after the amount scaling and the ±limit clamp. Because the reference is only 1.6 dB louder overall and tone-only is on, the tonal move is gentle — the biggest correction is +1.40 dB of air at 12 kHz — while the 4.5 dB of level lands in the trailing `volume` stage instead of being smeared across every band.

Switching the output to **ffmpeg equalizer command** with amount 50, smoothing 0 and Q 1.4 gives the same move at half strength:

```
ffmpeg -i input.wav -af "equalizer=f=63:t=q:w=1.4:g=0.20,equalizer=f=250:t=q:w=1.4:g=-0.80,equalizer=f=1000:t=q:w=1.4:g=-0.80,equalizer=f=4000:t=q:w=1.4:g=0.20,equalizer=f=12000:t=q:w=1.4:g=1.20" output.wav
```

And **CSV** returns the same derivation as machine-readable rows:

```
frequency_hz,track_db,target_db,diff_db,smoothed_db,gain_db
63,-12.00,-10.00,0.40,-0.60,-0.60
250,-8.00,-8.00,-1.60,-0.93,-0.93
1000,-6.00,-6.00,-1.60,-0.93,-0.93
4000,-10.00,-8.00,0.40,0.40,0.40
12000,-18.00,-14.00,2.40,1.40,1.40
```

### Input format

Frequency/level pairs, in any order, separated by newlines, commas, semicolons, colons, equals signs, pipes or plain spaces. Every one of these parses to the same three bands:

```
100 -10, 1k -6, 10k -14
100=-10; 1000=-6; 10000=-14
100Hz: -10dB
1kHz: -6dB
10kHz: -14dBFS
```

Frequencies accept a `k` multiplier and an optional `Hz` suffix; levels accept `dB`, `dBFS` or `dBr`. Bands are sorted by frequency automatically, so you can paste an analyser dump as-is.

### Target curves

| Curve | What it means |
| --- | --- |
| Pasted reference | Your reference levels, interpolated over log frequency onto your track's band frequencies |
| Flat | 0 dB at every band |
| Pink | −3 dB/octave from 1 kHz — equal energy per octave, the usual "balanced full-range mix" reference |
| Bright | −2 dB/octave — a shallower tilt with relatively more top end |
| Warm | −4 dB/octave — a steeper tilt with relatively more low end |
| Speech | Pink tilt plus a rolloff below 150 Hz, a +3 dB presence lift at 3 kHz and an air cut above 10 kHz |

The built-in curves are relative to 0 dB at 1 kHz, so leave **Tone only** on when you use one — otherwise the whole absolute level offset between your measurement scale and the curve gets baked into the band gains.

### Limits and edge cases

- The tool matches **tonal balance only**. It cannot copy dynamics, stereo image, saturation, ambience, arrangement or perceived "quality" — two spectra can be identical while the mixes sound nothing alike.
- Between 2 and 64 bands are accepted per curve. A 31-band third-octave RTA fits comfortably; beyond 64 bands you want convolution, not a filter chain.
- The reference is interpolated linearly in dB over log frequency, and held flat outside its measured range. If your reference stops at 8 kHz, everything above 8 kHz is matched to the 8 kHz value.
- Gains below 0.05 dB are dropped from the `equalizer` chain — a 0.04 dB peaking filter is a no-op that only costs a processing stage. The `firequalizer` output lists every band regardless, because it interpolates a continuous curve between the entries.
- The measurement itself happens upstream. Results are only as good as the analyser settings behind the numbers: use a time-averaged spectrum over a representative stretch of material (roughly 30 seconds is a common recommendation), not a single instantaneous frame.
- Large boosts lift whatever is already in the band, including noise and bleed. That is what **Maximum boost/cut** is for; the default ±6 dB is deliberately conservative.
- The loudness offset is a plain subtraction of the two LUFS figures you supply. Nothing is measured here, and applying it as a `volume` stage can clip if the track is already near full scale — check true peak after rendering.
- Both loudness fields default to 0, which means "skip the offset". If you genuinely measured 0 LUFS you have bigger problems than EQ.

## FAQ

<details>
<summary>Where do I get the band levels to paste in?</summary>

From any spectrum analyser, RTA or metering plug-in that can report per-band levels — most DAW analysers, standalone RTAs and measurement apps will show or export a table of centre frequencies with their levels. Use a time-averaged reading over a representative section rather than a single frame, and measure both your track and the reference the same way, on the same scale. The absolute scale does not matter as long as it is consistent, because tone-only matching removes the offset between them.

</details>

<details>
<summary>How is this different from a normal EQ or a tone control?</summary>

A normal EQ asks you to decide the gains. This tool derives them: it subtracts your measured curve from the target curve, smooths the difference, scales it by the amount, clamps it to the limit, and hands you the resulting band gains as a filter chain. It is a static, one-time correction — it does not track the signal over time the way a dynamic or adaptive EQ does.

</details>

<details>
<summary>Should I use 100 % amount?</summary>

Rarely on music. A full match copies every measurement quirk of the reference, including differences caused by arrangement rather than mixing — a reference with no low toms will tell you to cut your low toms. 50 % is a good starting point: it moves the balance clearly in the right direction while leaving the character of your mix intact. Full strength is more defensible for corrective work against a built-in curve, or when the two pieces of material are genuinely comparable.

</details>

<details>
<summary>What does smoothing actually change?</summary>

It averages each band's correction with its neighbours over the radius you choose. At 0 the correction follows your measurement exactly, narrow resonances included, which is what you want if you are chasing a specific room mode or a resonant peak. At 1–2 the result favours broad tonal balance, which is what you usually want for a mix match. At 4 the correction degenerates into a gentle tilt. In the worked example, the isolated 12 kHz difference of +2.40 dB becomes a +1.40 dB gain once ±1 band of smoothing shares it with its neighbour.

</details>

<details>
<summary>What is "tone only" for?</summary>

It separates *tone* from *level*. With it on, the average difference between the two curves is subtracted before matching, so the band gains describe the shape difference and nothing else — the level difference is reported separately and handled by the loudness offset. With it off, the broadband difference is baked into every band, which makes the whole chain act as a volume change plus an EQ. Leave it on whenever you are also using the LUFS fields or a built-in curve.

</details>

<details>
<summary>equalizer or firequalizer — which output should I use?</summary>

The `equalizer` chain is one peaking filter per band, so it is easy to read, easy to edit by hand, and easy to transcribe into a parametric EQ in your DAW. The `firequalizer` form is a single stage that interpolates a continuous curve through every band entry, which tracks the derived curve more faithfully and ignores the Q setting entirely. Use `equalizer` when you want to understand or tweak the move, `firequalizer` when you want the closest match to the computed curve.

</details>

<details>
<summary>Why did my reference and my track need different band counts?</summary>

They do not need to match. The reference is interpolated over log frequency onto your track's band frequencies, so you can match a 31-band third-octave measurement of your track against a 10-band octave measurement of the reference. Only your track's frequencies appear in the output, because those are the points where you actually have a measurement to correct.

</details>

<details>
<summary>Can it process the audio for me?</summary>

No — this tool works entirely on numbers and returns a command. Copy the generated `ffmpeg` line, point `-i` at your file, and run it locally; that keeps your audio on your machine and lets you re-render with a tweaked chain as many times as you like. You can equally transcribe the band gains into whatever parametric EQ you already use.

</details>
