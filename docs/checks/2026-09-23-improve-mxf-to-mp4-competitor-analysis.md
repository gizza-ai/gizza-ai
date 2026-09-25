# mxf-to-mp4 — competitor analysis (2026-09-23)

Scan run **before** implementing, per `create-next-tool`. All competitor notes are
**paraphrased observations of feature surfaces** — no competitor copy, branding, or
trademarks are reproduced or reused anywhere in this tool.

## Why this is not a duplicate of an existing block

The `X-to-mp4` family is heavily skiplisted in this repo as "source-container relabels"
(`ts-to-mp4`, `flv-to-mp4`, `m4v-to-mp4`, `mpeg-to-mp4`, `vob-to-mp4`, …). MXF is the one
case in that family where the existing blocks genuinely **do not** cover the job, verified
against real ffmpeg 7.1 runs on 2026-09-23:

| Existing block | Behavior on an MXF | Why it falls short |
| --- | --- | --- |
| `mkv-to-mp4` / `mov-to-mp4` (`mode=copy`) | `-c copy` **succeeds**, but writes the MXF's PCM audio into MP4 as `ipcm` | Verified: output streams are `mpeg2video/mp4v` + `pcm_s16le/ipcm`. `ipcm` is an ISO/IEC 14496-12 raw-PCM sample entry essentially no browser or consumer player decodes, so the "converted" file plays silent. The remux is not a usable MP4. |
| `mkv-to-mp4` / `mov-to-mp4` (`mode=transcode`), `video-to-h264`, `video-transcode` | Re-encodes the picture to libx264 | Correct output, but **always** re-encodes. MXF carrying AVC-Intra / XAVC / plain H.264 is already MP4-legal essence — re-encoding a broadcast master is a pointless generation loss. No block offers "keep the picture, fix only the audio". |
| `video-audio-track-selector` | `-c copy`, keeps one audio track | Still `-c copy` on audio → same unusable `ipcm` result, and it **discards** the other tracks rather than combining them. |
| `video-to-mxf` | The inverse direction | Not applicable. |

The two capabilities no block in the repo has, and that MXF specifically needs:

1. **Hybrid `-c:v copy -c:a aac`** — rewrap the already-MP4-legal picture essence while
   converting MXF's PCM audio to AAC. Every `-c:v copy` block in the repo
   (`video-audio-gain`, `video-mute`, …) is an audio *effect*, not a container conversion.
2. **Merging discrete mono audio tracks** (`amerge`). Broadcast MXF conventionally ships
   audio as 2–16 separate **mono** PCM tracks (channel-per-track), not one stereo track.
   Nothing in the repo merges them; the default result everywhere else is a single
   arbitrary mono channel.

Verified working argv (real ffmpeg 7.1, H.264-in-MXF with two mono PCM tracks):
`-i in.mxf -filter_complex "[0:a:0][0:a:1]amerge=inputs=2[a]" -map 0:v:0 -map "[a]" -c:v copy -c:a aac -ac 2 -movflags +faststart out.mp4`
→ `h264` video (untouched) + one 2-channel `aac` track.

## Competitors reviewed

Search: "MXF to MP4 converter online broadcast file convert". The top result
(CloudConvert's MXF→MP4 landing page) exposed no option detail on the public page, so it was
**replaced** with a fourth real result (online-convert) rather than running with fewer, per
the skill's rule.

### C1 — XConvert (MXF→MP4)
- Video codec choice: H.264 (default), H.265/HEVC, AV1, legacy MPEG-4.
- Quality control: multiple modes — quality presets, target file size, CBR, VBR, and
  constant-quality CRF (documents 18 ≈ visually lossless, 23 default, 28 small).
- Resolution: presets 144p–4320p, custom W×H, percentage scale, or keep original.
- Audio: AAC default; AC-3/E-AC-3/MP3/MP2/FLAC/Opus/Vorbis/PCM alternatives.
- Explicitly names the broadcast source families it accepts: Sony XDCAM HD/EX and XAVC,
  Panasonic AVC-Intra, Canon XF-AVC, ARRI.
- **States that broadcast-grade multi-track audio is dropped during conversion.** This is
  the exact gap our merge mode closes.
- No hard file-size cap advertised; suggests multi-GB files are hardware-bound.

### C2 — FreeConvert (MXF→MP4)
- Video codec: "Auto" or **"Copy"** (a no-re-encode path) — confirms remux is table stakes.
- Audio codec: "Auto" or "Copy"; volume 0–200%; fade in/out; remove audio entirely.
- Resolution (W×H), frame rate (with "no change"), rotate/flip.
- Trim by `HH:MM:SS.MS`, crop rectangle, subtitle burn-in/embed.
- **Max file size 1 GB** on the free tier, with a paid upgrade for more.
- UX: dropdowns for codecs, numeric fields for dimensions/volume, time spinners, checkboxes.
- No CRF/quality or audio-bitrate control exposed.

### C3 — online-convert (MXF→MP4)
- Video codec dropdown including h.264, h.265, av1, mpeg4, and **copy**.
- Bitrate (kbps), target file size (MB), frame rate, resolution with
  keep-aspect / stretch / no-upscale / pad / crop behaviors.
- Trim by `HH:MM:SS`, rotate 90/180/270, flip, per-edge crop, deinterlace checkbox.
- Audio: codec (aac/mp3/opus/vorbis/**copy**), **channels stereo-or-mono**,
  **bitrate 8–320 kbps**, sample rate 8 kHz–96 kHz, normalize, disable track.
- UX: dropdowns, numeric fields, checkboxes.

## Table stakes → decision

Every item lands in the descriptor or in the out-of-scope list below. Nothing dropped silently.

| Table stake | Seen in | Verdict | Where it lands |
| --- | --- | --- | --- |
| H.264 output as the default target | C1, C2, C3 | **in-model** | `picture = "h264"` (default): `-c:v libx264 -pix_fmt yuv420p -preset medium` |
| Copy / no-re-encode path | C2, C3 | **in-model** | `picture = "rewrap"`: `-c:v copy` (hybrid with AAC audio — see above) |
| CRF / constant-quality knob | C1 | **in-model** | `quality` 1–100 → CRF 40–18, rendered as a slider (repo-wide family convention) |
| AAC audio by default | C1, C2, C3 | **in-model** | always `-c:a aac` when audio is kept |
| Audio bitrate control | C3 (8–320 kbps) | **in-model** | `audio_bitrate` 32–320 kbps, default 192 |
| Stereo/mono channel control + remove audio | C2, C3 | **in-model** | `audio = "stereo" \| "merge" \| "all" \| "none"` |
| Multi-track broadcast audio | C1 (dropped), others silent | **in-model — our differentiator** | `audio = "merge"` + `merge_tracks` 2–16 via `amerge` → one stereo AAC |
| Faststart / streamable output | implicit in all | **in-model** | always `-movflags +faststart` |
| Accepts XDCAM / P2 AVC-Intra / XAVC / XF-AVC sources | C1 | **in-model** | ffmpeg's MXF demuxer handles all of them; documented on the page, no flag needed |
| Stated size limit | C2 (1 GB) | **in-model** | 10 MiB input / 10 MiB output cap, stated on the page rather than surfaced as an error |
| HEVC / AV1 output | C1, C3 | **considered, rejected** | Family invariant: `video-to-hevc` and `video-transcode` own those targets. Adding them here would duplicate two blocks. |
| Resolution / scale | C1, C2, C3 | **out of scope** | `video-resize` |
| Frame rate change | C2, C3 | **out of scope** | `video-fps` |
| Trim | C2, C3 | **out of scope** | `video-trim`, `video-cut-segments` |
| Rotate / flip / crop | C2, C3 | **out of scope** | `video-rotate`, `video-bake-rotation`, `video-crop` |
| Deinterlace | C3 | **out of scope** | `video-deinterlace` (broadcast MXF is often interlaced — cross-linked from our FAQ) |
| Target file size in MB | C1, C3 | **out of scope** | `video-target-filesize-encoder` |
| Volume / fades / normalize | C2, C3 | **out of scope** | `video-audio-gain`, `video-audio-fade`, `audio-normalize` |
| Subtitle burn-in / embed | C2 | **out of scope** | `video-caption-burner` |
| Accounts, paid tiers, GB-scale cloud batches | C1, C2, C3 | **out of model** | Browser-local wasm: no server, no account, no upload. The size cap is the honest trade. |

## UX patterns adopted

- Dropdowns for every fixed choice (`Param::enumv` → `<select>`), with `[input.labels]`
  giving plain-language labels instead of raw enum tokens.
- `kind = "slider"` for the 1–100 quality knob (C1 ships quality as a graded control).
- `[[example]]` preset chips standing in for competitors' preset menus, covering the four
  real workflows: default H.264, rewrap AVC-Intra, merge four mono tracks, picture-only.
- Limits, the `ipcm` pitfall, and the DNxHD/MPEG-2 rewrap failure are stated in the page FAQ
  rather than left to be discovered through an ffmpeg error.

## Known limitations (stated on the page)

- 10 MiB input / 10 MiB output — this is a browser-local wasm tool; real broadcast masters
  are far larger. Competitors allow 1 GB+ because they upload to a server.
- `rewrap` only works when the MXF's picture essence is MP4-legal (H.264/AVC-Intra/XAVC,
  HEVC). MPEG-2-based XDCAM HD/IMX and DNxHD essence cannot be rewrapped into MP4 — ffmpeg
  reports `Could not find tag for codec … not currently supported in container` (verified),
  and the fix is `picture = "h264"`.
- ffmpeg cannot be run in the chat Service Worker, so this tool's supported surfaces are the
  standalone page and the CLI.
