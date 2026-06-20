# video-mute — competitor analysis (2026-06-20)

Eighteenth `/create-next-tool` backlog pick (this iteration skiplisted
subtitle-translator [needs a model], trim-audio [audio-only, no Input::Audio],
video-convert [dup of video-transcode], video-merge [needs >1 media input]
before landing on a clean one). ffmpeg media tool — page + CLI. Research via
`WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| VidShift / ConvertPilot / WuTools | lossless stream-copy (strip audio, no re-encode), in-browser, no upload | capabilities |
| Canva / VEED / Movavi / Clideo | remove audio with no quality loss; keep resolution/format | capabilities |

## Gap diff vs our tool
Our tool: `-c:v copy -an` — stream-copies the video and drops the audio, so it's
lossless and fast (no re-encode), keeping the original container. This is exactly
the lossless stream-copy approach every competitor advertises as the headline.

**At parity — nothing to add this pass.** No parameters needed (the operation is
unambiguous: remove the audio).

**Out-of-model:** cloud imports (Drive/Dropbox), trimming/volume controls (those
are other tools — e.g. change-speed, video-crop already exist).

## Tested
unit (2: drops audio + lossless video copy, keeps input extension) + drift-guard ·
`wafer build` validates the block · wasm-pack web · generator · Playwright page
(uploads tiny-128x128.mp4 → in-browser ffmpeg → data:video) · CLI on a real public
video (ffprobe confirms 0 audio streams, 1 video stream in the output). Chat
ffmpeg: non-functional — page + CLI are the surfaces.

> Original work only — no competitor copy, branding, or trademarks copied.
