# video-to-prores — competitor / reference scan (2026-08-16)

Scan run **before** implementing `blocks/video-to-prores`, per `/create-next-tool` step 2.
All notes are **paraphrased**; no competitor copy, branding, or marketing text is reused.

## Sources inspected

| # | Source | Type | What it gave us |
|---|--------|------|-----------------|
| 1 | `appleprores422converter.com` (landing) | Desktop-converter marketing site | The full ProRes variant menu users expect; the 3-step Add → pick format → Convert flow; educational blocks on data rates / chroma / bit depth |
| 2 | `videoconverterfactory.com/tips/mov-to-prores.html` | How-to guide for a desktop converter | The six-variant family list (Proxy / LT / 422 / HQ / 4444 / 4444 XQ) with a plain-language "which one" framing; the FAQ questions real users ask |
| 3 | `videosolo.net/prores-converter/` | How-to guide for a desktop converter | Which variant is the common default (plain 422); the settings surface competitors expose (quality, resolution, bitrate, sample rate); "does converting improve quality?" FAQ framing |
| 4 | `brettleeper.com/tutorials/ffmpeg-mp4-mov/` | ffmpeg how-to | The minimal real command people actually run: `-c:v prores -profile:v 2`; the honest admission that the profile numbers are opaque and users guess |
| 5 | `ffmpeg -h encoder=prores_ks` (local, authoritative) | Encoder docs | Canonical profile names + numbers (`proxy 0 / lt 1 / standard 2 / hq 3 / 4444 4 / 4444xq 5`), supported pixel formats (`yuv422p10le`, `yuv444p10le`, `yuva444p10le`), `-vendor`, `-bits_per_mb`, `-quant_mat`, `-alpha_bits` |
| — | CloudConvert ProRes pages | (referenced by the search summary; the two direct URLs tried both 404'd, so it is recorded second-hand only) | Cloud converter with a per-day free processing quota, URL/cloud-drive import, and resolution/fps/bitrate/pixel-format fine-tuning |

## Table stakes observed

1. **Pick a ProRes variant.** Every source leads with the family, not a single codec. Proxy / LT / 422 / HQ are the 4:2:2 tier; 4444 / 4444 XQ are the 4:4:4 + alpha tier.
2. **A default that isn't a number.** Source 4 shows the real failure mode: ffmpeg users type `-profile:v 2` without knowing what 2 is. Named choices with a size/quality hint are the differentiator.
3. **`.mov` container, always.** Nobody offers ProRes in another wrapper; the QuickTime wrapper is the point (Final Cut Pro, Premiere, DaVinci Resolve, Avid).
4. **10-bit 4:2:2 is implicit.** No source exposes pixel format as a user choice for the 422 tier — it is a property of the codec tier, so it should be forced, not asked.
5. **Resolution control.** Sources 3 and the CloudConvert summary both expose a resolution/scaling knob. For ProRes specifically this is not cosmetic: the codec is ~147 Mbps at 1080p, so a downscale is the practical size lever.
6. **Audio handling.** Editorial workflows expect uncompressed PCM in the `.mov`, not a re-wrapped lossy track. No competitor asks the user, but every real ProRes recipe uses PCM.
7. **Educational framing.** All four web sources spend most of the page explaining *why* ProRes (edit-friendly intermediate, not a delivery codec) and *why the files are big*. That belongs in our page copy + FAQ, not in the schema.
8. **Privacy / no-upload.** The cloud option carries a daily quota and an upload; a browser-local converter's differentiator is that neither applies.

## Decisions — in-model (built into the descriptor)

| Table stake | Decision |
|---|---|
| Variant picker | `profile` — `Param::enumv` over `proxy \| lt \| standard \| hq`, default `standard`. Named, not numeric; each value's `.describe()` states the ~1080p data rate and the intended use. |
| Named default | Default `standard` = plain "ProRes 422", which source 3 calls the common pick. |
| `.mov` container | Forced. Output is always `out.mov` / `video/quicktime`; there is no container param. |
| 10-bit 4:2:2 | Forced `-pix_fmt yuv422p10le`. Not a user choice — it defines the 422 tier. |
| Resolution | `resolution` — `Param::enumv` over `source \| 2160p \| 1440p \| 1080p \| 720p \| 540p \| 480p`, default `source`. Implemented as a **downscale-only** `scale=-2:'min(ih,N)'`, so picking a height taller than the source is a no-op instead of an upscale. Values carry the `p` suffix deliberately: the ffmpeg page driver numeric-coerces numeric-looking field strings before `build_argv`, and a bare `720` would arrive as a JS number. |
| Audio | `audio` — `Param::enumv` over `pcm16 \| pcm24 \| none`, default `pcm16` (uncompressed 16-bit PCM, what editors expect). `pcm24` for 24-bit masters, `none` → `-an` for a picture-only intermediate. |
| QuickTime vendor tag | Forced `-vendor apl0` so the output identifies as Apple ProRes rather than the Lavc default — some editors are picky about the vendor atom. |
| Educational framing | Page copy + FAQ: what ProRes is for, why the file is much bigger than the source, why converting cannot recover quality the source never had. |
| Stated limits | Input/output caps and the "ProRes is huge" arithmetic are on the page, not just in code. |

## Decisions — considered, not built

| Feature | Why not |
|---|---|
| ProRes **4444 / 4444 XQ** | 4:4:4 chroma + a 16-bit alpha plane (`yuva444p10le`, `-alpha_bits`) is a different codec tier from 422 and a different user (VFX/compositing, not editorial). This tool's contract is the 422 tier — every output is a `apco/apcs/apcn/apch` 4:2:2 file. A sibling `video-to-prores-4444` is the honest shape; folding it in would make the forced `yuv422p10le` invariant conditional and the tool's name a lie. |
| ProRes **RAW** | Not an encoder ffmpeg ships at all (decode-only, and camera-originated). Impossible, not merely out of scope. |
| Frame-rate conversion | Already covered by `blocks/video-fps`; duplicating it here is schema bloat. Chain the two tools instead. |
| Explicit bitrate / `-bits_per_mb` | ProRes is a constant-quality codec whose rate is a property of the chosen tier. Exposing `-bits_per_mb` invites off-spec files that identify as ProRes but do not behave like it. The tier picker is the correct quality control. |
| `-quant_mat` override | Encoder-internal tuning; no competitor exposes it and a wrong value silently degrades the file. Left at `auto` (which follows the profile). |
| Batch conversion / queue | Desktop-converter feature. The page takes one upload; the CLI is the batch surface (`for f in *.mp4; do gizza tool video-to-prores …`). |
| Cloud-drive / URL import in the page | Cloud-converter feature that requires a backend. The CLI already accepts an HTTP(S) `url`; the page is deliberately local-file-only so nothing is uploaded. |
| `-movflags +faststart` | Deliberately omitted, unlike `mp4-to-mov`. Faststart exists for progressive web playback; a ProRes intermediate is never streamed, browsers cannot decode it, and the flag forces a full rewrite pass over a file that may be hundreds of MB. |

## Surface notes / limitations

- **Chat surface is not exercisable in this repo.** ffmpeg cannot run in the chat Service Worker (`import()`/`Worker` are SW-forbidden), so the supported surfaces are the standalone page and the CLI. The descriptor/drift-guard tests still validate what the chat schema would expose.
- **The output cannot be previewed in-browser.** No browser decodes ProRes, so the page renders a download link rather than a playing `<video>`. The Playwright spec therefore asserts correctness by parsing the produced MOV's bytes for the codec fourcc (`apco`/`apcs`/`apcn`/`apch`) instead of decoding it.
- **Size is the real constraint.** ProRes 422 is roughly 147 Mbps at 1080p29.97 — about 18 MB per second of video. A measured 2 s 320×240 encode is 1.4 MB. Caps are set accordingly (32 MiB in / 128 MiB out on the chat+CLI path) and the page states the arithmetic so users pick `proxy` or a lower `resolution` for anything long.
