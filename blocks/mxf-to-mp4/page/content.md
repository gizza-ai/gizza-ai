## Convert MXF to MP4 in your browser

**MXF** (SMPTE 377M, Material Exchange Format) is the container broadcast gear
writes: Sony XDCAM HD and XAVC, Panasonic P2 AVC-Intra, Canon XF-AVC, IMX/D-10,
Avid DNxHD. Editors and playout servers read it happily; browsers, phones,
Premiere-less laptops and every "attach a video" box on the web do not. This
tool rewraps or re-encodes an MXF into an ordinary `.mp4` using ffmpeg compiled
to WebAssembly — the file is processed on your own machine and never uploaded.

### Why an MXF needs more than a container swap

Dragging MXF through a plain `-c copy` remux looks like it works and usually
doesn't, for two reasons this tool handles explicitly:

- **MXF audio is uncompressed PCM.** Copied into MP4 it lands as `ipcm`, a raw
  PCM sample entry almost no browser or consumer player decodes — the
  "converted" file plays with silent audio. So audio here is **always re-encoded
  to AAC**, whichever mode you pick.
- **Broadcast MXF stores audio as separate mono tracks**, one channel per track:
  2, 4, 8, sometimes 16 of them. Keeping "the" audio track gives you one
  arbitrary mono channel — the left half of a stereo mix, or a single mic. The
  **Merge** mode folds the first N tracks back into one normal stereo track.

### Picture: re-encode or rewrap

- **Re-encode to H.264 (default)** — `libx264` at 8-bit `yuv420p`. Always
  produces a playable MP4, and it's the *only* option for MPEG-2-based essence
  (XDCAM HD, IMX/D-10) and for DNxHD, which MP4 cannot legally carry. The
  quality slider (1–100, default 75) maps onto ffmpeg's CRF scale: 100 ≈ CRF 18,
  visually lossless; 75 ≈ CRF 24; 1 ≈ CRF 40, small and soft.
- **Rewrap picture** — `-c:v copy`. The picture essence is moved into the MP4
  byte-for-byte: no generation loss, no re-compression, near-instant. This works
  when the MXF already holds MP4-legal essence — AVC-Intra, XAVC, plain H.264,
  HEVC — and errors out on anything else. Audio is still converted to AAC, so
  this is a hybrid: keep the master's picture, fix only the sound.

### Audio modes

| Mode | What you get |
| --- | --- |
| **First track as stereo** (default) | Track 1 only, encoded as one 2-channel AAC track. |
| **Merge mono tracks** | The first *N* tracks (2–16) combined with `amerge` and downmixed to one stereo AAC track. The broadcast case. |
| **Keep every track** | Each source track becomes its own AAC stream at its native channel count — useful when tracks carry an alternate language or a clean-effects bed. |
| **No audio** | Picture only. |

AAC bitrate is adjustable from 32 to 320 kbps (default 192). Drop to 96–128 for
speech, raise to 256+ for music.

### Worked example

An XDCAM clip `A001C003.mxf` holds AVC-Intra picture plus four discrete mono
audio tracks (dialogue L, dialogue R, boom, radio mic). Choose **Rewrap
picture**, set audio to **Merge mono tracks** with **4** tracks, and you get
`A001C003.mp4`: the identical AVC-Intra video stream, untouched, plus a single
192 kbps stereo AAC track containing all four channels mixed down — playable in
any browser, and the picture never re-compressed.

Ask for more tracks than the file actually has and the conversion fails with a
clear ffmpeg "matches no streams" error, rather than quietly mixing a partial
result.

### Limits

- Input and output are each capped at **10 MiB**. This runs in your browser, not
  on a render farm — real broadcast masters are far bigger, so cut a section out
  first with a trim tool.
- **Rewrap** needs MP4-legal picture essence. On MPEG-2 (XDCAM HD, IMX) or
  DNxHD, ffmpeg reports `Could not find tag for codec … not currently supported
  in container`; switch to **Re-encode to H.264**.
- The MXF's timecode and data tracks are dropped; the output is a single video
  stream plus AAC audio, written with `+faststart` so it can play while
  downloading.
- Interlaced broadcast material stays interlaced — deinterlacing, resizing,
  frame-rate conversion and trimming are separate tools, not bundled here.

## FAQ

<details>
<summary>Why does my MXF convert but come out silent elsewhere?</summary>

Because the audio was stream-copied instead of re-encoded. MXF carries
uncompressed PCM, and copying it into an MP4 writes it as `ipcm` — a raw PCM
sample entry that browsers and most consumer players cannot decode, so the file
looks fine and plays silent. This tool never copies audio: every mode that keeps
sound re-encodes it to AAC, which plays everywhere.

</details>

<details>
<summary>My MXF has 4 or 8 audio tracks. Which one do I keep?</summary>

Usually none of them on their own — use **Merge mono tracks** and set the track
count to match. Broadcast MXF stores audio channel-per-track, so each "track" is
one mono channel of a larger mix. Merging 2 gives you back the L/R pair; merging
4 or 8 folds a multi-channel layout down to one stereo track. If the tracks are
genuinely independent (an alternate language, a music-and-effects bed), pick
**Keep every track** instead and let the player choose.

</details>

<details>
<summary>What's the difference between rewrap and re-encode?</summary>

**Rewrap** copies the picture into the MP4 untouched (`-c:v copy`): no quality
loss at all, and it finishes almost instantly because nothing is compressed.
**Re-encode** runs the picture through libx264 again, which is slower and loses
a little detail, but always succeeds. Use rewrap when the MXF already holds
H.264-family essence — AVC-Intra, XAVC, plain H.264 or HEVC — and re-encode for
everything else.

</details>

<details>
<summary>Rewrap failed with "Could not find tag for codec". Why?</summary>

The MXF's picture essence isn't something MP4 can hold. XDCAM HD and IMX/D-10
are MPEG-2, and DNxHD is its own codec; neither has a legal MP4 sample entry, so
ffmpeg refuses the mux rather than writing a broken file. Switch the picture
option to **Re-encode to H.264** and it will convert.

</details>

<details>
<summary>Does the quality slider affect a rewrap?</summary>

No. It only controls libx264 when you re-encode; a rewrap copies the existing
picture stream, so there is no encoder to tune. The AAC bitrate setting applies
in both modes, because audio is always re-encoded.

</details>

<details>
<summary>Is my footage uploaded anywhere?</summary>

No. The conversion runs entirely inside your browser with ffmpeg compiled to
WebAssembly. The MXF never leaves your device, there is no account and no
server-side processing, and the page keeps working offline once it has loaded.

</details>
