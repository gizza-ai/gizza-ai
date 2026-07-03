# video-aspect-pad — competitor analysis (2026-07-03)

Two passes. §1 is the build-time quick scan (kept for the record). §2 is the
/improve-tool deep scan: 5 real competitor tool pages, one read-only research
subagent each, profiled for params/defaults/output/UX/SEO, limits and
free-vs-paid positioning. All competitor observations are paraphrased — no
copy, branding or assets were taken.

## 1. Build-time quick scan (original)

One WebSearch ("letterbox video to 9:16 aspect ratio online tool pad video with color reels
shorts"); skimmed the top real tools: EditClips change-video-aspect-ratio, Turbo Digital
aspect tools (Fit / Fill / BlurPad), RenderFire aspect-ratio converter, VidStudio resize
(letterbox mode), Typito vertical-video editor, Clipchamp aspect-ratio guide. Findings and
the original design decisions are unchanged below (§3 documents what the improve pass
changed).

## 2. Improve-pass deep scan (top 5)

### 2.1 EZGIF — change-aspect-ratio (ezgif.com/change-aspect-ratio)
- **Model:** server upload (200 MB cap, temp files auto-deleted ≤1 h), free, ad-funded,
  no watermark, no account.
- **Params:** 11 ratio presets (1:1, 4:3, 16:9, 3:2, 2:1, 1:2, 2:3, 3:4, 4:5, 5:4, 9:16;
  no custom ratio), method = stretch | pad (bars fixed BLACK — no color choice, no blur),
  plus a separate crop control incl. auto-detect-and-remove existing black bars; output =
  original container | MP4 | WebM, no quality knob.
- **Output quality:** always re-encodes (even a no-op run); padding EXTENDS the canvas at
  source size (320×240 → 426×240 for 16:9 — only approximately the ratio).
- **UX:** two-step upload→form flow, no live preview; result inline with before/after size
  delta badge; chaining toolbar into ~20 sibling tools; upload by file/paste/drag/URL.
- **SEO angles (paraphrased):** free no-signup ratio change; remove black bars; pad-vs-
  stretch explainer; platform pairing (1:1 Instagram, 9:16 TikTok/Shorts); GIF/APNG/WebP
  supported too; JSON-LD WebApplication with price 0.

### 2.2 EditClips — change-video-aspect-ratio (editclips.online)
- **Model:** browser-local ffmpeg-wasm (same architecture as ours), free core, no
  watermark/account; soft upsell (credits, server processing, API); 100 MB/file free cap.
- **Params:** preset grid 16:9, 9:16, 1:1, 4:3, 3:2, 21:9 + custom W:H inputs; fit method
  = stretch | crop | letterbox (bars fixed black — NO color control); quality tier =
  quick/standard/best (no numbers exposed); container = MP4/MKV/MOV/WebM.
- **UX:** live pre-processing preview, batch multi-file drop, paste-to-upload (Ctrl+V),
  3-card fit-mode picker with tradeoff labels, 4-step how-to, sibling single-purpose tool
  cluster (4:3→16:9, make-square, …).
- **SEO:** platform pairing copy; stretch-vs-crop-vs-letterbox education; privacy/no-upload;
  long-tail sibling pages. No worked pixel-math examples (open angle we keep).

### 2.3 WuTools — change-aspect-ratio (wutools.com/video/change-aspect-ratio)
- **Model:** browser-local ffmpeg-wasm, fully free, no watermark/account; 100 MB cap;
  ~30-60 s per 5 min of video claimed.
- **Params:** presets 16:9 (default, 1920×1080), 9:16, 4:3, 1:1, 4:5, 21:9 + custom ratio;
  fit = crop (default) | pad | stretch; pad background = black | white | **blurred copy of
  the video**; format = same | MP4 (H.264/AAC) | WebM (VP8/Vorbis); quality = 3 tiers
  mapped to **explicit CRF 18/23/28** (H.264) or bitrates (WebM).
- **Output:** even dimensions, yuv420p, **faststart/moov-front on MP4/MOV** ("social
  players start streaming instantly" pitch).
- **UX:** file metadata card (name/size/duration/detected ratio), platform-labeled presets
  with pixel dimensions, conditional controls (pad color only in pad mode), FAQ accordion.
- **SEO:** repurpose one master into every platform's shape; crop-vs-pad-vs-stretch
  guidance; letterbox/pillarbox explainer with blur pitched for landscape→portrait; naming
  the exact CRF as a trust signal.

### 2.4 Clideo — resize-video (clideo.com/resize-video)
- **Model:** SERVER upload/cloud processing; free to 500 MB; third-party reviews report a
  watermark on free exports, removed on Pro (~$9/mo); account optional; cloud-storage import.
- **Params:** per-platform AND per-ad-format size presets (Stories/Reels/IGTV, FB cover,
  YouTube 240p-2160p + ad formats, Snapchat, Twitter, Pinterest, LinkedIn) + custom W/H
  (one dimension auto-derives the other); fit = pad | fill-crop; padding fill = suggested
  color swatches + free hex + **blur**; zoom + drag positioning; 20+ output containers.
- **UX:** 3-step wizard; swatch+hex+blur fill picker; preview before download; Trustpilot
  trust signals; heavy sibling cross-links. TikTok absent from the page (their gap).
- **SEO:** resize-for-platform head terms; vertical↔landscape with blurred background;
  crop-vs-resize education; phone/photo support FAQs.

### 2.5 VidStudio — resize (vidstudio.app/resize)
- **Model:** browser-local ffmpeg-wasm + WebCodecs decode; fully free, no watermark, no
  signup; no hard caps (device RAM bound); teaches users to verify no-upload in DevTools.
- **Params:** modes resize | letterbox | crop; presets 9:16 1080×1920, 16:9 1920×1080 and
  1280×720, 1:1 1080×1080, 4:5 1080×1350, 2:3 1000×1500 (platform-labeled); custom W/H;
  keep-ratio toggle; letterbox bar color choice (default black, "any color", no blur);
  crop X/Y offsets. Output locked to H.264 MP4, no quality knob.
- **UX:** preset buttons grouped per platform, mode switcher, no live preview.
- **SEO:** per-platform dimension table; resize-vs-letterbox-vs-crop explainer; 4K→1080p
  recipe; platform tips (Shorts 60 s cap etc.); privacy trust walkthrough.

## 3. Gap list (ours vs the five) and decisions

| # | Gap (≥1 competitor does it, we didn't) | Seen at | Dim | Tag | Decision |
|---|---|---|---|---|---|
| 1 | Blurred-background fill for the bars | WuTools, Clideo (+Turbo "BlurPad" in §1) | capabilities | in-model | **BUILT** — `blur` boolean; split/cover-scale/crop/boxblur/overlay graph in core; radius scales with canvas (`min(w,h)/16`, ≥2) |
| 2 | Quality tiers with explicit CRF | WuTools (CRF 18/23/28), EditClips (unnumbered) | capabilities | in-model | **BUILT** — `quality` enumv high/medium/low ↔ CRF 18/23/28, default medium (family default kept) |
| 3 | faststart/moov-front MP4 for instant social streaming | WuTools | capabilities | in-model | **BUILT** — `-movflags +faststart` on .mp4/.mov outputs |
| 4 | 3:2 ratio preset | EZGIF, EditClips | capabilities | in-model | **BUILT** — `3:2` (1620×1080) added to the enum |
| 5 | Color picker widget (swatch + hex) | Clideo swatches+hex; VidStudio color choice | ux | in-model | **BUILT** — declarative `kind = "color"` (existing shared control); text stays canonical so names stay expressible |
| 6 | Platform-labeled presets with pixel dimensions | WuTools, VidStudio, Clideo | ux/seo | in-model | **BUILT** — new declarative `[input.labels]` in the shared generator (option VALUES stay canonical); applied to aspect + quality |
| 7 | One-click platform presets | Clideo/VidStudio preset buttons | ux | in-model | **BUILT** — five `[[example]]` chips (Reels, Blur 9:16, Square white, Cinematic, Pinterest) |
| 8 | Paste-to-upload (Ctrl+V a copied file) | EditClips, EZGIF | ux | in-model | **BUILT** — generic in shared `tool.js` ffmpeg driver (accept-class matched; text pastes untouched) |
| 9 | Crop-vs-pad-vs-stretch decision guidance | EditClips, WuTools, EZGIF | copy | in-model | **BUILT** — new FAQ entry; blur positioned as the middle path |
| 10 | "Verify no-upload in DevTools" trust copy | VidStudio | copy | in-model | **BUILT** — added to the privacy FAQ (original wording) |
| 11 | Runnable CLI/deep-link examples (found broken: the color placeholder leaked into the generated examples as `color=black, white or #1A2B3C`, which core rejects) | (our own regression, caught by the waveform-image lesson) | ux | in-model | **FIXED** — `kind="color"`'s non-hex-placeholder omit rule now drops color from the examples; example verified verbatim via CLI |
| 12 | Crop / stretch fit modes | all five | capabilities | out-of-scope | not built — single-purpose tool family: this tool is the pad mode; stretch distorts by design |
| 13 | Free-form custom ratio (any W:H) | EditClips, WuTools, Clideo | capabilities | not built (design) | enumv keeps the page a `<select>` and the LLM schema tight; `width` override covers exact canvases; 2:1/1:2/5:4 presets judged too rare (EZGIF-only) |
| 14 | Output container/format choice (MP4/WebM/MKV) | EZGIF, WuTools, EditClips, Clideo | capabilities | not built (scope) | transcoding is a sibling-tool job; we keep the input container; VP8/9 encoder availability in the wasm ffmpeg build unverified |
| 15 | Batch multi-file | EditClips | ux | out-of-model v1 | shared ffmpeg page driver is single-file by design |
| 16 | Zoom/reposition content in the frame | Clideo | capabilities | out-of-scope | pad keeps the whole frame centered by definition; repositioning belongs to a crop/compose tool |
| 17 | Auto-detect & remove existing black bars | EZGIF | capabilities | out-of-scope | separate tool idea (bar-detect + crop), noted for the backlog |
| 18 | Cloud processing, accounts, paid tiers, 100 MB-500 MB caps | EZGIF, Clideo, EditClips upsell | — | out-of-model | gizza is browser-local/no-account; our 25 MB limit stated on-page |

## 4. Original design decisions (build pass, unchanged)

- **Canvas semantics, not pad-only:** output is always exactly the target canvas
  (default per aspect: 9:16→1080×1920, 1:1→1080×1080, 16:9→1920×1080, 4:5→1080×1350,
  3:4→1080×1440, 4:3→1440×1080, 3:2→1620×1080 (new), 2:3→1080×1620, 21:9→2520×1080; or
  `width` × derived height). Exact, even output dims make the result platform-ready and
  testable, and match how the competitors' "9:16 preset" actually behaves (EZGIF's
  source-size padding yields only-approximate ratios — §2.1 — validating this choice).
- `width` must be even (H.264/yuv420p), 16–4096; odd/out-of-range is rejected with a
  guiding error, not clamped (family invariant). Derived height is rounded to even.
- `scale` gets `force_divisible_by=2` so a rounding edge can never overflow the pad area
  and the content box stays even. `setsar=1` forces square pixels.
- Color is validated in core against ffmpeg's own 140-name color table plus `#RGB`/
  `#RRGGBB` hex (normalized to `0xRRGGBB`); anything else is rejected with the expected
  shapes in the message. Strict charset keeps the filtergraph injection-free.
- Audio is stream-copied (`-c:a copy`) — padding never touches sound (also in blur mode:
  `-map 0:a?` keeps it optional).
- Verification proves GEOMETRY and BARS, not just "a video came out": page specs decode
  the output via `<video>`+canvas and pixel-assert exact dims, bar colors (incl. short
  hex) and surviving content; blur mode asserts mid-brightness non-uniform bars with the
  foreground stripes intact; the CLI matrix ffprobes every preset's canvas and
  pixel-checks named/short/full/bare hex colors, blur, quality-tier size ordering,
  moov-before-mdat, and all guided error paths.
