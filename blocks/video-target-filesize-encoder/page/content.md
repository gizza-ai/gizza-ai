## About this tool

Some places put a hard cap on how big a video can be: Discord's 10 MB free upload,
a 25 MB email attachment, WhatsApp's 16 MB limit. This tool re-encodes your clip so
the result lands **under the MB budget you pick** — no trial and error.

It works from the file-size formula `size ≈ (video_bitrate + audio_bitrate) × duration ÷ 8`.
It reads your clip's length, subtracts the audio you asked for, and computes the H.264
video bitrate that fits the remaining budget (with a small safety margin so the muxed
MP4 stays under the cap). Then it does one encode to **MP4 (H.264 + AAC)** for maximum
compatibility. Everything runs in your browser — the file is never uploaded.

### Worked example

A **12-second, 640×480** clip, target **1 MB**, audio **128 kbps**:

- budget in bits: `1 MB × 8 × 0.95 ≈ 7,969,178`
- audio takes: `128,000 × 12 = 1,536,000`
- video bitrate: `(7,969,178 − 1,536,000) ÷ 12 ≈ 536 kbps`

The encoder is told `-b:v 536k -maxrate 536k -bufsize 1072k`, and the output lands around
**0.5–0.9 MB** — comfortably under the 1 MB cap (highly-compressible footage undershoots
more). Set **Audio: No audio** to spend the whole budget on the picture, or lower **Max
resolution** when a full-resolution clip can't reach a small target.

## FAQ

<details>
<summary>Does it guarantee the file is exactly the target size?</summary>

No — it guarantees the file lands **at or under** your target, which is what upload limits
actually need. It uses single-pass bitrate targeting with a small safety margin, so
easy-to-compress clips (talking heads, screen recordings) often come out noticeably
smaller than the cap. It will not overshoot into "just over the limit" territory.

</details>

<details>
<summary>Is this two-pass encoding?</summary>

No. It's a **single-pass** encode with a computed `-b:v` plus `-maxrate`/`-bufsize`
(constant-bitrate style). True two-pass VBR would allocate bits a little more cleverly,
but the in-browser ffmpeg bridge runs one encode per click and can't carry a pass-log
file between two runs, so single-pass is the honest scope here. In practice, for hitting
an upload limit, the difference is small.

</details>

<details>
<summary>What if the target is too small for my clip?</summary>

If the budget minus the audio can't leave a usable video bitrate (below ~50 kbps), the
tool tells you so instead of producing garbage. Fixes: raise the target a little, lower
the **Max resolution** (e.g. 720p or 480p), or set **Audio** to *No audio*. Longer clips
need a bigger budget for the same quality, because the bits are spread over more seconds.

</details>

<details>
<summary>What formats can I put in, and what comes out?</summary>

Input can be MP4, MOV, MKV, WebM and other common formats your browser can decode. The
output is always **MP4 (H.264 video + AAC audio)** — the most widely-supported combination
for Discord, email, and messaging apps. WebM/other containers are re-encoded to MP4.

</details>

<details>
<summary>Does my video get uploaded anywhere?</summary>

No. The encode runs entirely in your browser via WebAssembly ffmpeg. Your file never
leaves your device, and there's no size-based paywall. Very large inputs are limited only
by your browser's available memory.

</details>
