## Remove electrical hum from a video

A steady low buzz under your audio — the classic 50 Hz or 60 Hz **mains hum**
picked up from power supplies, ground loops, cheap adapters and fluorescent
lights — sits at one fundamental frequency *and* a stack of harmonics above it.
This tool drops a **narrow notch filter** on the fundamental and on each
harmonic in one pass, pulling the hum comb out while leaving the rest of the
audio alone. The picture is **stream-copied** (not re-encoded), so it stays
byte-for-byte identical and processing is fast; only the audio is re-encoded.
Everything runs in your browser — nothing is uploaded.

### Pick the mains frequency

- **50 Hz** — Europe, Asia, Africa, most of Australia. Harmonics land at 100,
  150, 200 Hz…
- **60 Hz** — North America and most of South America. Harmonics land at 120,
  180, 240 Hz…

If you don't know, look at where the clip was recorded, or just try both and
keep the one where the buzz disappears.

### Harmonics and narrowness

- **Harmonics to notch** (0–12) — how many multiples above the fundamental to
  also filter. Hum is rarely a pure tone; it leaks into 2×, 3×, 4×… the base.
  The default **4** notches, for 50 Hz, 50 / 100 / 150 / 200 / 250 Hz. Set it to
  **0** to notch only the fundamental.
- **Notch narrowness (Q)** (1–100) — how tight each notch is. A **higher Q** is
  narrower and removes less of the audio around each hum line (safer for music
  and voices); a lower Q is wider and more aggressive. **10** is a good general
  default; **2–10** suits most mains hum, and **30–40** is a good music-safe
  choice.

**Worked example:** a webcam interview with a steady 50 Hz buzz from a nearby
power brick. Load `interview.mp4`, leave **Mains frequency** on **50 Hz**,
**Harmonics** on `4` and **Q** on `10`. Notches at 50/100/150/200/250 Hz pull
the buzz down while the voice stays intact, and you get
`interview-dehummed.mp4`.

### Notes and limits

- The video stream is copied losslessly; the output keeps the same container
  (mp4 → mp4, webm → webm). WebM audio is re-encoded to Opus, everything else to
  AAC.
- This is a classic notch chain, not an AI "studio voice" model — it targets
  *steady* mains hum sitting at fixed frequencies, not broadband hiss or fan
  noise. For general hiss, use the **Remove Background Noise from a Video** tool.
- One setting is applied to the whole clip; there is no per-region control. Trim
  first if you only want part of the audio processed.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Should I choose 50 Hz or 60 Hz?</summary>

Match the electrical grid where the clip was recorded: **50 Hz** for Europe,
Asia, Africa and Australia, **60 Hz** for North and South America. If you're not
sure, try one, and if the buzz is still there switch to the other.

</details>

<details>
<summary>What are "harmonics" and how many should I notch?</summary>

Mains hum is never a single pure tone — energy also appears at whole-number
multiples of the base frequency (for 50 Hz: 100, 150, 200 Hz…). **Harmonics to
notch** filters those too. The default **4** clears the fundamental plus the
first four multiples, which handles most hum. Raise it if a higher-pitched buzz
survives; set it to **0** to touch only the fundamental.

</details>

<details>
<summary>What does the Q (notch narrowness) control do?</summary>

Q sets how tight each notch is. A **high Q** (say 30–40) is very narrow, so it
removes the hum line with almost no effect on nearby music or voices. A **low
Q** (2–10) is wider and more aggressive — better when the hum drifts slightly or
is strong, at the cost of thinning the audio around it. **10** is a sensible
default.

</details>

<details>
<summary>Will removing hum hurt the video quality?</summary>

No. Only the audio is changed; the **picture is stream-copied without
re-encoding**, so the video quality is identical to the original.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with a
`-dehummed` suffix (e.g. `clip.mp4` → `clip-dehummed.mp4`). The input and output
are each capped at 25 MB.

</details>
