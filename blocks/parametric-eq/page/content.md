## About this tool

Parametric EQ lets you boost or cut exact frequency ranges instead of using fixed bass/mid/treble tone controls. Each of the three bands has:

- **Frequency** — the centre of the band in Hz, from 20 to 20000.
- **Gain** — how much to boost or cut, from -20 dB to +20 dB. A gain of `0` turns that band off.
- **Q** — bandwidth. Low Q values are wide and gentle; high Q values are narrow and surgical.

For example, set band 2 to frequency `3000`, gain `4`, Q `1.5` to add vocal presence. To reduce mud, set band 1 around `250`, gain `-5`, Q `1.2`. To add air, set band 3 around `10000`, gain `4`, Q `0.7`.

The output is re-encoded to MP3 by default, or WAV, OGG, FLAC, or M4A if you choose another format. Album-art/video streams are dropped so the result is a clean audio file.

### Limits

At least one band must have a non-zero gain; otherwise there is nothing to process. The tool applies three peaking EQ bands, not shelves, high-pass filters, or a live spectrum analyzer. Very large source files are capped by the standard audio upload limit, and boosted bands can cause clipping if the original audio already peaks near full scale — cut other bands or normalize first when in doubt.

## FAQ

<details>
<summary>How is this different from the regular audio equalizer?</summary>

The regular audio equalizer has fixed bass, mid, and treble frequencies. This parametric EQ lets you choose the exact frequency and Q of every band, so it can target a narrow resonance, a specific vocal range, or a custom tonal shape.

</details>

<details>
<summary>What Q value should I use?</summary>

Use a low Q such as `0.7` for broad musical tone shaping, around `1` to `2` for normal corrective moves, and higher values such as `5` or more for narrow notches. Extremely narrow high-Q boosts can sound harsh, so use them carefully.

</details>

<details>
<summary>Why does a 0 dB band do nothing?</summary>

A gain of 0 dB means no boost and no cut, so that band is omitted from the ffmpeg filter chain. This keeps the output cleaner and avoids unnecessary filters.

</details>

<details>
<summary>Can boosting a band clip the audio?</summary>

Yes. Any positive gain raises part of the signal and may push peaks over full scale. If you hear distortion, reduce the boost, cut surrounding bands instead, or run a normalization/limiting tool after EQ.

</details>

<details>
<summary>Which output format should I choose?</summary>

Use MP3 for compact sharing, WAV or FLAC for lossless workflows, OGG for open web audio, and M4A when AAC compatibility is important. Re-encoding lossy input to another lossy format can reduce quality, so keep lossless sources when possible.

</details>
