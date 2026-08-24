# video-to-mxf — competitor analysis (2026-08-23)

Scan performed before implementation. One web search ("convert video to MXF online broadcast
XDCAM HD422 IMX D-10 converter"), then the top reachable results were skimmed. All notes are
paraphrased observations of capability; no competitor copy, branding or trademarked wording is
reproduced or reused on the tool page.

## Landscape

The search surfaced three usable classes of competitor and one dead end:

1. **Browser converters** (e.g. the XConvert MXF converter). Reachable and representative.
2. **Broadcast-oriented ffmpeg forks** (FFmbc, `github.com/umc-live/FFmbc`). Reachable, but the
   README is the only public documentation — the option tables it references are not published,
   so parameter defaults had to be derived from the specs themselves rather than quoted.
3. **Desktop MXF converters** (Mac/Windows shareware aimed at XDCAM camera cards). Reachable
   marketing pages, thin on parameters.
4. **Hardware clip servers** (1U SDI recorders that write XDCAM-compatible MXF). Out of scope —
   a hardware product, not a comparable tool; noted and dropped.

The most important finding is directional: **almost every "MXF converter" on the open web converts
_out of_ MXF, not _into_ it.** Camera-card MXF → MP4/MOV/ProRes is the mass-market job. Writing a
conformant MXF delivery file is treated as a professional NLE / encoder feature (Adobe Media
Encoder, Vegas, FFmbc), and the forum thread found in the search is a user discovering their
consumer editor simply cannot export XDCAM HD422 or IMX/D-10 at all. That is the gap this tool
fills, and it is why the tool is container-first rather than a general converter.

## Table-stakes parameters observed

| Capability | Where seen | Decision here |
| --- | --- | --- |
| Named broadcast profile (XDCAM HD422 / XDCAM HD / IMX / D-10) | FFmbc README lists XDCAM HD422 and XDCAM IMX/D-10 as first-class output targets, in both MOV and MXF | **In model.** `profile = xdcam_hd422 \| xdcam_hd \| imx50`, defaulting to the 50 Mbps 4:2:2 HD tier. |
| Rewrap without re-encoding | Implied by every "my file is fine, the container is wrong" support thread | **In model.** `profile = copy` (`-c:v copy`), with explicit errors when the user also asks to rescale or retime. |
| Target resolution: keep original, preset list, or custom W×H | Browser converters expose all three | **Partly in model.** Presets `auto` / `source` / `1920x1080` / `1280x720`. Free-form W×H is deliberately excluded: an arbitrary raster is by definition not a delivery spec, and D-10 rejects anything but its own. |
| Frame-rate selection | Broadcast specs are stated as rate + raster together; the forum thread shows users being handed a spec sheet | **In model.** `frame_rate` with the eight broadcast rates plus `source`, using true `N/1001` fractions. |
| Bitrate control (CBR vs VBR, target rate) | Browser converters expose CBR/VBR and a target; broadcast specs mandate CBR | **In model but not user-exposed.** The bitrate and rate-control mode are properties of the profile (50/35 Mbps CBR), so exposing them would only let a user produce a non-conformant file. Documented on the page instead. |
| Audio codec choice + bit depth | Browser converters offer AAC/MP3/PCM/…; broadcast specs mandate 48 kHz PCM | **Narrowed on purpose.** `audio = pcm16 \| pcm24 \| none`, always 48 kHz. AAC is not offered because MXF cannot carry it. |
| Batch conversion | XConvert converts several files in parallel | **Out of model.** One file per page run; the CLI covers scripted batches. |
| Cloud-drive import (Drive/Dropbox) | XConvert | **Out of model** for the page (local-only by design); the CLI/chat surfaces accept any HTTP(S) URL. |
| Trim by time range | XConvert | **Out of model here** — already covered by the trim/cut tools in this toolkit. |
| Privacy claim ("files deleted after a few hours", "encrypted upload") | XConvert | **Bettered by architecture.** Nothing is uploaded at all on the page; stated plainly in the hero and FAQ rather than as a retention promise. |
| Loudness normalisation to EBU R128 / −23 LUFS | Raised as a hard blocker in the broadcast forum thread — the station required it alongside the MXF spec | **Out of model for this tool.** It is a real delivery requirement, but it belongs to an audio-loudness tool, not the wrapper; noted here so it is not silently dropped. |
| Timecode / closed-caption / metadata track authoring | Mentioned in FAQ material about camera MXF | **Out of model.** ffmpeg's MXF muxer will not author conformant ancillary-data tracks; listed as a stated limit on the page. |
| OP-Atom (Avid per-track) MXF | Called out in competitor FAQ material as a P2/Avid requirement | **Out of model.** `mxf_opatom` carries exactly one stream per file, so it cannot produce a single video+audio deliverable; listed as a stated limit. |
| AVC-Intra / XAVC targets | FFmbc README | **Out of model.** No ffmpeg encoder produces conformant class-tagged AVC-Intra essence; listed as a stated limit. |

## UX controls worth copying (as patterns, not copy)

- Competitors present the profile as a **named preset**, not as a pile of codec knobs. Mirrored:
  the page's primary control is one `<select>` of named delivery profiles, with the bitrate,
  pixel format, GOP and muxer all implied by the choice.
- Preset chips: the page ships four `[[example]]` chips (HD422 1080 25p, XDCAM HD 35 at 720p
  29.97, IMX 50 SD, and rewrap) so the common specs are one click.
- Friendly `<select>` labels spell out what each value means in delivery-spec language
  ("MPEG-2 4:2:2, 50 Mbps CBR") while the submitted values stay canonical.

## Worked example recorded

`xdcam_hd422` + `auto` + `25` + `pcm16` on a 4K 30 fps clip →
`mpeg2video (4:2:2), yuv422p, 1920x1080, 25 fps, 50 Mbps CBR` + `pcm_s16le, 48000 Hz`, in an OP1a
MXF. This is reproduced verbatim in the page's "About this tool" section as the input→output
example.

## Findings that changed the design

Two constraints were discovered by probing ffmpeg directly rather than from any competitor:

1. **D-10 is 25 fps only here.** ffmpeg derives a fixed D-10 frame size from bitrate × time base;
   50 Mbps yields 250,000 bytes at 25 fps but 208,541.67 at 30000/1001, and every 525/60 raster
   (720×486, 720×512, 720×608) is rejected at packet-mux time. The tool validates this up front.
2. **CBR needs a real raster.** `-minrate = -maxrate` on a small source frame makes the MPEG-2
   encoder fail to open ("impossible bitrate constraints"), so `resolution = source` falls back to
   average-VBR and is documented as non-conformant.

Both are stated on the page rather than left for the user to hit as an ffmpeg abort.
