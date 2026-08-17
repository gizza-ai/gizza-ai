## About this tool

A de-esser reduces the short, harsh bursts that make **s**, **sh**, **t** and **ts** sounds jump out of a vocal, narration or podcast track. It is different from a normal EQ cut: the high band is ducked dynamically only while sibilance is present, so quiet breaths and the general brightness of the voice stay closer to the original.

This tool uses ffmpeg's dedicated `deesser` filter. The controls are exposed as honest 1-100 scales because the filter's `amount`, crossover and reduction controls are unitless coefficients, not fixed Hz or dB values. Start with the default vocal preset, then use **ess mode** to listen only to what is being removed; if you hear too much of the voice body in that audition track, raise the band or lower the amount.

Example settings for a vocal WAV:

```bash
gizza tool de-esser 'url=https://example.com/vocal.wav' amount=60 band=70 max_reduction=50 mode=output format=mp3
```

For a gentler podcast polish, try amount 35, band 80 and max reduction 40. For troubleshooting, switch mode to `ess` and format to `wav`; the output should mostly contain the harsh esses, not the whole voice.

Limits and edge cases: this is not broadband noise removal, a click remover or a full multiband mastering chain. Very aggressive settings can create a lisp or dull consonants. Embedded album art/video streams are dropped because the audio is re-encoded.

## FAQ

<details>
<summary>Is this the same as lowering 6 kHz with an equalizer?</summary>

No. A static EQ cut lowers that band for the whole file, which can make the entire vocal dull. This de-esser ducks the upper band dynamically only when sibilance is detected. Use `audio-eq` when you really want a constant tone change.

</details>

<details>
<summary>What should I listen for in ess mode?</summary>

Ess mode renders only the material being removed. Ideally you hear mostly sharp consonants and very little vowel or voice body. If the audition output sounds like the full vocal, raise `band`, lower `amount`, or lower `max_reduction`.

</details>

<details>
<summary>Why are the band controls not labeled in Hz?</summary>

ffmpeg's `deesser` filter uses unitless coefficients whose exact crossover depends on the source sample rate. A fixed Hz label would be misleading, so the page uses 1-100 controls and describes the practical direction: higher band values keep the effect closer to the very top of the spectrum; lower values treat more of the voice as sibilance.

</details>

<details>
<summary>Which output format should I choose?</summary>

Use MP3 for sharing, WAV or FLAC when you plan to keep editing, OGG for web projects, and M4A when you need an AAC-style file. De-essing rewrites the samples, so every format is newly encoded.

</details>
