# Competitor analysis — video-remove-embedded-subs (2026-07-18)

Function: strip all embedded **soft** subtitle/caption streams from a video container while
keeping the video and audio streams intact (remux, no re-encode).

Search: "remove embedded subtitles from video online tool strip subtitle track" (WebSearch,
2026-07-18). The reachable market splits into two very different product classes — this
matters for fit-to-model.

## Top reachable competitors scanned

1. **VideoProc — "How to Remove Subtitles From Video (Hardcoded & Soft Subs)"**
   (videoproc.com/video-editor/remove-subtitles-from-mp4.htm). The most on-topic reference:
   explicitly distinguishes **soft** subtitles (a separate stream inside the container that
   can be removed cleanly) from **hardcoded** subtitles (burned into the frame). For soft
   subs the documented approach is exactly a stream-drop remux; for hardcoded subs it falls
   back to crop or AI. Table-stakes it establishes for the soft-sub path: remove the subtitle
   stream, keep A/V untouched, no re-encode, support MP4/MKV/MOV/AVI.

2. **HitPaw Edimakor — "How to Remove Subtitles from Video | MKV, MP4, AVI, MOV"**
   (edimakor.hitpaw.com). Positions around container coverage (MKV/MP4/AVI/MOV) and handling
   both embedded (soft) and hardcoded subtitles. Reinforces broad container support and an
   output-format choice as table stakes.

3. **Media.io AI Subtitle Remover / RecCloud / Vmake / Wink.ai / CreatOK**
   (anieraser.media.io, reccloud.com, vmake.ai, wink.ai, creatok.ai). This whole cluster is
   **AI hardcoded-subtitle removal** — they detect the on-screen subtitle region and inpaint
   the background to erase burned-in text. Cloud-based, quota/login/watermark tiers, clip
   length caps (e.g. CreatOK ~60s).

## Table stakes → our decision

| Capability | In model? | Decision |
|---|---|---|
| Remove all soft subtitle/caption streams | yes | **core** — `-map 0 -map -0:s -sn -c copy` |
| Keep video + audio (no re-encode, lossless) | yes | **shipped** — `-c copy` |
| Broad container support (MP4/MKV/MOV/WebM/AVI) | yes | **shipped** — `keep` preserves input; ffmpeg reads all |
| Choose/convert output container | yes | **shipped** — `container = keep\|mp4\|mkv` |
| Preserve attachments (fonts) + data streams | yes | **shipped** — negative subtitle map keeps everything else |
| Local / private (no upload) | yes | **shipped** — browser wasm ffmpeg, nothing uploaded |
| Worked example + stated limits + FAQ | yes | **shipped** — page copy |

## Out-of-model (listed, not built)

- **AI hardcoded/burned-in subtitle removal** (the Media.io/RecCloud/Vmake/Wink/CreatOK
  cluster). Needs subtitle-region detection + generative inpainting — an ML model. gizza is
  pure-Rust + ffmpeg with no model runtime, so this is out of model. The page states clearly
  that only soft subtitle streams are removed and burned-in subtitles cannot be.
- **Remove a *specific* subtitle track by index/language** while keeping others. A reasonable
  future refinement, but the tool's stated purpose is to strip *all* embedded subtitles;
  per-track selection would need track enumeration UX (ffprobe-style) that the single-upload
  page model doesn't expose. Considered, not built.
- **Cloud batch / accounts / API keys.** Backend features, out of the browser-local model.

No competitor copy, branding, or trademarks were reproduced — this is a paraphrased feature
scan only.
