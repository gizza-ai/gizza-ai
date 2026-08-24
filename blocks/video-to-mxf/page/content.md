## About this tool

Use this when a station, archive, playout server, or finishing workflow asks for an MXF deliverable instead of an MP4 or MOV. The tool writes an MXF file with one of the named broadcast profiles: **XDCAM HD422** (MPEG-2 4:2:2 at 50 Mbps), **XDCAM HD** (MPEG-2 4:2:0 at 35 Mbps), **IMX 50 / D-10** for SD 625/50 delivery, or a **rewrap** mode that copies an already-compliant video stream into MXF while converting audio to PCM.

A typical delivery run starts with a camera MP4 or MOV, chooses **XDCAM HD422**, sets **Frame rate** to **25**, leaves **Resolution** on **Auto**, and keeps **Audio** at **16-bit PCM**. The output is an OP1a MXF with `mpeg2video`, `yuv422p`, a 1920×1080 padded delivery raster, 50 Mbps CBR video, and 48 kHz PCM audio. Sources are scaled to fit the chosen raster and padded with black; they are not cropped or stretched.

Choose **Source rate** only when you need a best-effort wrapper for odd frame rates. Strict MXF delivery specs usually name a broadcast rate such as 25, 29.97, 50, or 59.94. The page runs locally in your browser; the CLI/chat path caps input at 32 MiB and output at 160 MiB to avoid surprise memory use.

## Limits and edge cases

- MXF files are mezzanine/delivery files, not web playback files. Download the result and inspect it in an editor, media inspector, or `ffprobe`.
- **IMX 50 / D-10** is locked to 720×576 at 25 fps. Other D-10 rasters and 525/60 IMX combinations are rejected up front because ffmpeg's D-10 muxer cannot packetise them cleanly here.
- **Resolution = Source** keeps the source size and therefore uses average-VBR video. Pick **Auto**, **1920×1080**, or **1280×720** for strict CBR delivery-style output.
- **Rewrap video** cannot rescale or retime the picture. Use it only when the source video essence is already acceptable for the receiving system.
- This tool does not author timecode, closed captions, OP-Atom MXF, AVC-Intra, XAVC, or loudness-normalised audio. Prepare those in a dedicated tool before or after this wrapper.

## FAQ

<details>
<summary>Which MXF profile should I pick?</summary>

Use **XDCAM HD422** for the common 50 Mbps 4:2:2 HD house standard, **XDCAM HD** when a lighter 35 Mbps 4:2:0 file is explicitly requested, and **IMX 50 / D-10** only for SD 625/50 deliverables. Use **Rewrap video** only when the source stream already meets your spec and the container is the problem.

</details>

<details>
<summary>Why is audio always PCM or none?</summary>

MXF delivery files normally carry 48 kHz PCM audio, and MXF cannot carry common web audio such as AAC in the way an MP4 does. The tool offers 16-bit PCM, 24-bit PCM, or no audio so the output stays predictable for broadcast-style workflows.

</details>

<details>
<summary>Why does the source frame-rate option mention unofficial MXF?</summary>

ffmpeg's MXF muxer accepts only a fixed set of broadcast edit rates by default. Keeping an unusual source rate requires relaxing that check, which can make a useful wrapper but not a strict delivery file. Pick an explicit broadcast rate when a spec sheet names one.

</details>

<details>
<summary>Does this replace an NLE export preset?</summary>

No. It covers a focused MXF wrapping/transcode step with named MPEG-2 broadcast profiles. It does not create ancillary tracks, captions, slate metadata, loudness reports, or broadcaster-specific QC packages that a full finishing workflow may require.

</details>
