## About this tool

The **Audio Waveform Video Generator** turns any audio track into an animated
waveform MP4 — the "audiogram" clips podcasters and musicians post to Instagram,
TikTok, X and YouTube. Upload an MP3 (or WAV, FLAC, OGG, M4A, Opus — anything
ffmpeg decodes) and a moving waveform is drawn from the sound while the original
audio is muxed back in, so the wave and the audio stay perfectly in sync.

Everything runs **in your browser** with WebAssembly ffmpeg — your audio is never
uploaded to a server.

Pick a **style** — *mirror* (a centered, symmetric line: the classic audiogram
look), *bars* (one vertical bar per sample), *wave* (a continuous line) or
*points* (a dot per sample). Set the **frame size** for the platform you're
posting to (1280×720 for a 16:9 landscape video, 720×1280 for a 9:16 vertical
story/Reel, 1080×1080 for a square feed post), choose the wave **color** and an
optional second color for a left-to-right **gradient**, set the **background**,
and tune the **frame rate**. If your audio is quiet, switch the **amplitude
scale** from *linear* to *sqrt*, *cbrt* or *log* to boost the motion so the wave
keeps dancing.

### Worked example

Make a 9:16 vertical story clip from a podcast snippet with cyan bars on a dark
blue background, boosted so quiet speech still moves:

- **Waveform style:** Bars — vertical bar per sample
- **Width / Height:** 720 × 1280
- **Wave color:** `#22d3ee`
- **Background color:** `#0b1220`
- **Amplitude scale:** Square root — boost quiet audio
- **Frame rate:** 30

Drop in your audio and the tool renders an MP4 the exact length of the clip,
ready to upload. On the command line the same job is:

```
gizza tool audio-waveform-video 'url=https://example.com/podcast.mp3' 'mode=bars' 'width=720' 'height=1280' 'color=#22d3ee' 'background=#0b1220' 'scale=sqrt' 'fps=30'
```

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: site/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. -->

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The waveform is rendered entirely in your browser with a WebAssembly build
of ffmpeg. Your audio file never leaves your device — nothing is uploaded to a
server.

</details>

<details>
<summary>What audio formats can I use?</summary>

Anything ffmpeg can decode: **MP3, WAV, FLAC, OGG, M4A/AAC and Opus** all work.
The output is always an **MP4** (H.264 video + AAC audio) so it plays everywhere
and uploads cleanly to social platforms. Files up to about 10 MB work best in the
browser.

</details>

<details>
<summary>Which size should I pick for Instagram, TikTok or YouTube?</summary>

Match the platform's aspect ratio: **1280×720** (16:9) for a YouTube/landscape
video, **720×1280** (9:16) for a TikTok, Instagram Reel or Story, and
**1080×1080** (1:1) for a square feed post. The three preset chips above set
these for you. Any size from 64 up to 3840×2160 (4K) is allowed.

</details>

<details>
<summary>The wave barely moves on quiet audio — how do I fix it?</summary>

Change the **Amplitude scale** from *Linear* to *Square root*, *Cube root* or
*Logarithmic*. Linear draws the true waveform, so quiet passages look flat; the
other scales progressively boost low amplitudes so the wave keeps moving even on
soft speech or ambient music.

</details>

<details>
<summary>Does the waveform stay in sync with the audio?</summary>

Yes. The waveform is generated frame-by-frame from the audio and the **original
audio track is muxed back into the MP4**, so the motion always matches the sound.
The video is exactly as long as the audio (`-shortest`).

</details>

<details>
<summary>Can I make a color gradient wave?</summary>

Yes. Set a **Gradient end color** in addition to the wave color and the wave is
filled with a horizontal left-to-right gradient from the first color to the
second. Leave it empty for a solid-color wave. All colors accept `#RGB`,
`#RRGGBB` or `#RRGGBBAA` (with alpha) hex.

</details>

## Limits & notes

- Output is always an opaque MP4 (H.264 + AAC) — MP4 has no transparency, so a
  solid background is always drawn.
- Stereo audio is downmixed to mono so you get one clean wave, not two channels
  blended on top of each other.
- Best for clips up to ~10 MB of audio; very long tracks at 60 fps and 4K will
  be slow to render in the browser.
- Styles are linear (`showwaves`); radial/circular visualizers are not supported.
