# pin-image-resizer — competitor analysis (2026-07-10)

Function: resize and crop any image to one of Pinterest's recommended pin formats
(standard 2:3 1000×1500, square 1000×1000, tall/infographic 1000×2100, story 9:16
1080×1920), with a fit mode (cover / contain / stretch), crop gravity (center / top /
bottom) for cover, and a pad background colour for contain — entirely in the browser
via ffmpeg (WASM), no upload, no account, free.

## Competitors skimmed (paraphrased; no copy/branding reproduced)

- **Adobe Express — Pinterest resize feature.** Web tool that takes a JPG/PNG, lets
  you pick a Pinterest preset or drag a size slider, then crop/scale/pan before
  download. Sits inside a broader multi-platform resizer (Instagram, Facebook, X,
  YouTube). Usable without an account for basic resizing but nudges toward sign-in for
  extra editing. SEO angle: brand authority plus a per-platform landing page that
  funnels into the wider editor.
- **ImResizer — resize image for Pinterest.** Closest analog: resizing runs
  client-side for privacy, no login/watermark. Six named presets (standard 1000×1500,
  square 1000×1000, idea 1080×1920, long/infographic 1000×2100, board cover 600×600,
  profile). Three fit modes it labels cover (crop edges to fill), contain (full image
  with padding), and manual crop. Accepts JPG/PNG/WebP/PDF, batch up to 12 images,
  ~20 MB cap. SEO angle: privacy / "in-browser, no login" plus batch.
- **Instasize — Pinterest image resizer.** Simple upload → pick format → download flow
  with three presets (pin 1000×1500, profile 800×800, board cover 600×600). Accepts
  JPG/PNG/WebP/HEIC. Free with "no signup" messaging but an app/account upsell;
  includes a size cheat-sheet. SEO angle: format cheat-sheet content plus a mobile-app
  funnel.
- **iLoveIMG — general image resizer.** Not Pinterest-specific but ranks for the
  query; resize by pixels or percentage with a keep-aspect-ratio checkbox and a
  "don't enlarge if smaller" toggle. No true crop/gravity or pad colour — it fits
  within max dimensions only. JPG/PNG/SVG/GIF, bulk supported. SEO angle: broad
  tool-suite domain authority.
- **Canva — Pinterest resize / Magic Resize.** Template-first editor rather than a
  one-shot resizer; users start from a Pinterest-sized canvas and export. One-click
  multi-size resizing (Magic Resize) is an AI-assisted paid feature requiring an
  account. Rich manual crop/position and background fill. SEO angle: template
  galleries and "create a Pinterest pin" intent, monetised via subscription.

## Table-stakes → decision (in-model = shipped in the descriptor)

| Capability | Competitor pattern | Decision |
|---|---|---|
| Named pin presets (standard/square/tall/story) | ImResizer, Instasize, Adobe Express expose per-format presets | **in-model** — shipped: standard 1000×1500, square 1000×1000, tall 1000×2100, story 1080×1920 |
| Cover (scale + crop to fill) | ImResizer "cover"; Adobe crop-to-ratio | **in-model** — shipped as `fit=cover` (default) |
| Contain (scale + pad) | ImResizer "contain"; iLoveIMG fit-within | **in-model** — shipped as `fit=contain` |
| Stretch (ignore aspect) | Rare; most tools avoid it | **in-model** — shipped as `fit=stretch` |
| Crop gravity for cover (center/top/bottom) | Manual crop region (ImResizer, Adobe, Canva) approximates this | **in-model** — shipped as `gravity=center/top/bottom`; a fixed-gravity subset of manual crop, sufficient for pin framing |
| Pad background colour for contain | Implicit white/transparent in most; Canva offers fill colour | **in-model** — shipped as `background` (#hex or colour name, default white) |
| Board-cover / profile presets (600×600, 165/280, 800×800) | ImResizer, Instasize | **out-of-scope** — non-pin avatar/cover sizes; belong in a generic image-resize tool, not the pin tool |
| Batch / bulk (up to 12 images) | ImResizer, iLoveIMG | **out-of-scope** — batching is a cross-tool concern, not this single-image descriptor |
| Manual free-crop rectangle / pan-and-zoom | Adobe, Canva, ImResizer | **out-of-scope** — interactive canvas cropping belongs to `image-crop`; fixed gravities cover the pin-framing need |
| AI background-extend / outpaint to fit ratio | PixExtender, Canva Magic | **out-of-model** — needs generative ML inference; gizza has no ML loader or server |
| One-click AI multi-size / Magic Resize | Canva | **out-of-model** — account + cloud/AI dependency |
| 1000×3000 (1:3) infographic variant | some Pinterest sizing guides | **considered, rejected** — Pinterest crops much past ~1:2.1 in-feed, so 1000×2100 is the practical long-pin max; a 1:3 preset would mostly ship a pin that gets truncated |

## Pinterest dimensions (authoritative)

Cross-referenced against Pinterest sizing guides (Tailwind 2025 chart, LouiseM,
PinGenerator) and search consensus:

- **Standard pin: 1000×1500 px, 2:3** — Pinterest's own optimal recommendation; images
  outside 2:3 risk truncation in-feed. **Our preset matches.**
- **Square pin: 1000×1000 px, 1:1** — valid pin/carousel format. **Our preset matches.**
- **Long / infographic pin: 1000×2100 px, ~1:2.1** — recommended long-pin size; some
  guides cite a taller variant up to 1000×3000 (1:3), but the feed crops much past
  ~1:2.1. **Our preset matches the mainstream long-pin number.**
- **Idea / story pin: 1080×1920 px, 9:16** — the single recommended full-screen mobile
  size. **Our preset matches.**

No corrections needed to our four preset numbers.

Sources: Adobe Express (Pinterest resize), ImResizer (resize for Pinterest),
Instasize (Pinterest resizer), iLoveIMG (resize image), Tailwind (Pinterest image
size chart 2025), LouiseM (best Pinterest pin size), PinGenerator (pin dimensions).

## Worked example baked into the tool

Take a landscape 4000×3000 (4:3) photo and target the **standard** pin (1000×1500,
2:3) with `fit=cover` and `gravity=center`: the image is scaled so its shorter side
fills the frame, then the overflowing left/right edges are cropped symmetrically,
yielding a full-bleed 1000×1500 pin with no padding. Switch to `fit=contain` with a
pad colour of `#ffffff`: the whole photo is scaled to fit inside 1000×1500 and the
empty top/bottom bands are filled white, preserving the entire composition. Change
gravity to `bottom` under cover to keep the lower portion (useful when the subject
sits low). `fit=stretch` would distort the 4:3 photo to fill 2:3 exactly — offered
but rarely desirable.

## Surfaces

ffmpeg-WASM image tool with a dedicated page, so all three gizza surfaces apply and
are verified:

- **Chat / LLM API** — invoked by `descriptor()` with named params (preset, fit,
  gravity, background), returning the resized-image media envelope. Drift-guard test
  pins the chat schema.
- **CLI** — same operation headless via `gizza tool pin-image-resizer url=… preset=…`.
- **Page / Playwright** — the tool page accepts query-params (preset, fit, gravity,
  background) and is checked end-to-end with Playwright, running ffmpeg in-browser
  with no upload and no account.
