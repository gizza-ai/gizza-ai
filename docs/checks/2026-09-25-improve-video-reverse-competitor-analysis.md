# video-reverse — competitor analysis (2026-09-25)

Scan run BEFORE implementing `blocks/video-reverse`. Every competitor below was read for
*capabilities, parameters, defaults and UX patterns only* — all copy here is paraphrased, and no
competitor wording, branding or asset was reused.

## Competitors reviewed

| # | Tool | Reach |
|---|------|-------|
| 1 | Ezgif — Reverse Video | reachable |
| 2 | Clideo — Reverse Video | reachable |
| 3 | Kapwing — Reverse Video | reachable (full editor, reverse is one action in it) |
| 4 | videoreverser.com — Online video reverser | reachable |

(A fifth, Picsart's video-studio reverse page, was skimmed from search results only and adds
nothing beyond the four above: upload → reverse → optional speed → download, no watermark.)

## What each ships

**Ezgif.** The most parameterised of the four. Audio is a three-way choice — keep the original
sound, reverse the sound along with the picture, or mute entirely. Offers a boomerang/ping-pong
mode (plays forward, then back). Keeps the source resolution/encoding and falls back to MP4 for
unusual inputs. Accepts a long container list (MP4, WebM, AVI, MPEG, MKV, FLV, OGG, MOV, M4V, WMV,
ASF, 3GP) and a direct file URL as well as an upload. 200 MB cap; server-side, files deleted after
an hour.

**Clideo.** Reverse plus three playback speeds (normal / slow / fast). Audio can be kept or muted.
Claims frame-rate and resolution preservation. Upload or cloud picker, preview before download,
optional format conversion afterwards. No stated size cap; server-side with account prompts.

**Kapwing.** Reverse is an action inside a timeline editor: trim, speed, volume, text/animation
layers, real-time preview, then export. No documented reverse-specific options, formats, or caps;
accepts Vimeo/TikTok links.

**videoreverser.com.** Minimal: upload or URL, an output-format dropdown (mp4 / mpg / mov / wmv), a
"remove audio track" checkbox, 10 MB upload cap. No boomerang, no speed.

## Table stakes → where each landed

| Table stake | Seen on | Decision |
|---|---|---|
| Reverse picture **and** sound together | all four | **In descriptor** — default `mode=reverse`, `audio=reverse` (`-vf reverse` + `-af areverse`) |
| Three-way audio handling (reverse / keep forward / mute) | Ezgif, (Clideo, videoreverser: keep+mute only) | **In descriptor** — `audio = reverse\|keep\|mute` |
| Boomerang / ping-pong (forward then back) | Ezgif | **In descriptor** — `mode = forward-reverse`, plus the mirror `reverse-forward` (reversed half first) |
| Preview before download | Clideo, Kapwing, Ezgif | **Already ours** — the generated page renders the result in a `<video>` player with a download link |
| Upload *or* direct URL input | Ezgif, videoreverser | **Already ours** — the page takes an upload, the CLI/chat schema takes `url` or `ref` |
| Quality/size control over the re-encode | none expose it (Clideo only *claims* preservation) | **In descriptor** — `quality = high\|balanced\|small` (CRF 18/23/28), because reversing always forces a re-encode and the user should own that trade-off |
| Stated input cap | Ezgif 200 MB, videoreverser 10 MB | **In descriptor + page** — 25 MiB, stated on the page with the reason (`reverse` buffers every decoded frame in memory) |
| No watermark / no signup / no upload | all four claim some of these | **Already ours, and stronger** — the page runs ffmpeg in the browser, so the file never leaves the machine |
| Preset one-click starting points | Ezgif/Clideo mode buttons | **In page** — four `[[example]]` chips (backwards, boomerang, riser, silent reverse) |

## Considered, rejected (in-model but declined)

- **Playback-speed control** (Clideo's slow/fast reverse, Kapwing's speed action, Picsart's speed
  slider). Technically one `setpts`/`atempo` pair away, but this repo already ships a dedicated
  `change-speed` block, and the repo's own skiplist rejected `video-speed` as a duplicate of it.
  Duplicating the knob here would fork the same capability across two tools; chain the two instead.
- **Output-container / format picker** (videoreverser's mp4/mpg/mov/wmv dropdown). Reversing forces
  a full re-encode, so this tool always writes H.264/AAC MP4 — the one container every accepted
  input can be re-encoded into and every browser can play back from a `data:` URL. Container
  conversion is `video-transcode`'s job.
- **Trim / crop / text overlays / timeline editing** (Kapwing). Out of scope for a single-purpose
  tool; `video-trim`, `video-crop` and `video-title-card` already cover these individually.

## Out of model (listed, not built)

- **200 MB-class inputs** (Ezgif). Those tools re-encode on a server; ours runs in the browser
  (and in a wasm CLI block), where `reverse` must hold every decoded frame of the clip in memory.
  25 MiB is the honest cap, and the page says so.
- **Cloud/Drive/Dropbox pickers, account-bound projects, social-link import** (Clideo, Kapwing).
  Require a backend and a login; gizza is local-only, no-account.
- **Real-time scrubbing preview while editing** (Kapwing). Needs a server-rendered proxy/timeline
  editor, not a single-shot browser transform.

## Net delta vs the field

Ours matches the parameter depth of the most capable competitor (Ezgif: audio three-way + boomerang)
and adds the mirrored `reverse-forward` build-up and an explicit quality control that none of the
four expose, while being the only one of the five that never uploads the file.
