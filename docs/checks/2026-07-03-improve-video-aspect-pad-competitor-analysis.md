# video-aspect-pad — competitor analysis (2026-07-03)

One WebSearch ("letterbox video to 9:16 aspect ratio online tool pad video with color reels
shorts"); skimmed the top real tools: EditClips change-video-aspect-ratio, Turbo Digital
aspect tools (Fit / Fill / BlurPad), RenderFire aspect-ratio converter, VidStudio resize
(letterbox mode), Typito vertical-video editor, Clipchamp aspect-ratio guide.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Aspect presets 9:16 / 1:1 / 16:9 / 4:3 | all | in-model | `aspect` enumv, default 9:16 (the Reels/Shorts headline case) |
| More social presets (4:5, 3:4, 2:3 Pinterest, 21:9 cinema) | Typito (20+ formats), letterbox template packs | in-model | included in the enumv (8 presets total) |
| Letterbox = fit whole frame, bars fill the rest | all ("Fit"/"letterbox" mode) | in-model | `scale=…:force_original_aspect_ratio=decrease` + centered `pad` |
| Pad color: black default, white, or any brand color | Turbo Digital, VidStudio, RenderFire | in-model | `color` param — 140 CSS/X11 names (ffmpeg's own table) or hex `#RRGGBB`/`#RGB`, default black |
| Platform-standard canvas (1080×1920 for 9:16 etc.) | RenderFire/Typito presets | in-model | per-aspect default canvas; optional `width` overrides it (height follows the ratio) |
| Local / in-browser, no upload, no watermark | EditClips, VidStudio | in-model | how gizza pages work; stated on page |
| Blurred-background pad ("BlurPad") | Turbo Digital | out-of-model v1 | needs split/boxblur/overlay graph; listed, not built (CSV sibling image-aspect-pad mentions it too) |
| Crop-to-fill / stretch modes | EditClips, Turbo Digital | out-of-scope | crop is `video-crop`; stretch distorts — this tool is the letterbox/pad mode only |
| Free-form custom ratio (any W:H) | EditClips | out-of-model v1 | enumv keeps the page a `<select>` + the LLM schema tight; the `width` override covers exact-size needs |
| Color PICKER widget | most editors | out-of-model | page generator has no `kind="color"` control (falls back to text); text field + placeholder documents names/hex |

## Design decisions

- **Canvas semantics, not pad-only:** output is always exactly the target canvas
  (default per aspect: 9:16→1080×1920, 1:1→1080×1080, 16:9→1920×1080, 4:5→1080×1350,
  3:4→1080×1440, 4:3→1440×1080, 2:3→1080×1620, 21:9→2520×1080; or `width` × derived
  height). Exact, even output dims make the result platform-ready and testable, and match
  how the competitors' "9:16 preset" actually behaves. The pad-only-at-source-size
  alternative can't guarantee the exact ratio (even-dimension rounding) and yields
  unpredictable sizes.
- `width` must be even (H.264/yuv420p), 16–4096; odd/out-of-range is rejected with a
  guiding error, not clamped (family invariant). Derived height is rounded to even.
- `scale` gets `force_divisible_by=2` so a rounding edge can never overflow the pad area
  ("Input area not within the padded area" ffmpeg failure) and the content box stays even.
- Color is validated in core against ffmpeg's own 140-name color table (`ffmpeg -colors`,
  case-insensitive) plus `#RGB`/`#RRGGBB` hex (normalized to `0xRRGGBB`); anything else is
  rejected with the expected shapes in the message. Strict charset also keeps the
  filtergraph string injection-free. `setsar=1` forces square pixels so players honor the
  padded geometry.
- Audio is stream-copied (`-c:a copy`) — padding never touches sound.
- Verification proves the GEOMETRY and the BARS, not just "a video came out": the page
  spec decodes the output via `<video>`+canvas and asserts exact canvas dims (90×160 for
  9:16@width=90), red bar pixels top+bottom, and the fixture's yellow center still present
  (letterbox); the deep-link case does the pillarbox axis (16:9@128 → 128×72, white side
  bars, non-white center). CLI check pads a public 320×176 clip to 1:1 and ffprobes
  320×320 + bar/center pixel colors.
