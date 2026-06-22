# android-asset-generator — competitor analysis (2026-06-22)

## What we built

`gizza-ai/android-asset-generator` takes one source image (url⊕ref) and returns a ZIP
laid out as an Android `res/mipmap-*` resource tree:

| Bucket            | Size    | Ratio (mdpi=1) |
|-------------------|---------|----------------|
| `mipmap-mdpi`     | 48×48   | 1.0×           |
| `mipmap-hdpi`     | 72×72   | 1.5×           |
| `mipmap-xhdpi`    | 96×96   | 2.0×           |
| `mipmap-xxhdpi`   | 144×144 | 3.0×           |
| `mipmap-xxxhdpi`  | 192×192 | 4.0×           |
| `play-store-icon.png` | 512×512 | — (Play listing) |

Optional `name` (default `ic_launcher`, sanitized to a valid Android resource name) and
`round=true` (adds a circular-masked `<name>_round.png` per bucket — the `ic_launcher_round`
variant). High-quality Lanczos3 downscaling. Pure Rust (`image` + `zip`) → runs on every
backend incl. the chat Service Worker. Surfaces: chat + CLI (no page — ZIP-of-images output
fits neither the text nor the media page render shape, like extract-pdf-images).

The density sizes and the mipmap (not drawable) folder convention were verified against the
official Android developer docs and corroborating sources (see below); the 48/72/96/144/192
ladder + 512 Play icon is exactly the set Android's own Image Asset Studio emits.

## Competitors surveyed

1. **Android Studio — Image Asset Studio** (built-in). Generates per-density launcher icons
   into `res/mipmap-*`, plus legacy/round/Google-Play-Store icons. Also produces **adaptive
   icons** (API 26+) from separate foreground + background layers, with circle/squircle/
   rounded-square shape previews.
2. **Android Asset Studio** (romannurik.github.io / jgilfelt fork) — web launcher-icon
   generator: per-density PNGs in a downloadable ZIP, padding/shape/background-color options,
   adaptive-icon layer support.
3. **EasyAppIcon / AppIcon.co / MakeAppIcon** — upload one square image, download a ZIP of
   all Android (and iOS) density buckets. Core feature = exactly what we do.
4. **appicon.co–style CLI/npm generators** (e.g. `app-icon`, `cordova-res`) — generate the
   density ladder from one source, sometimes both platforms.

## Gap diff & ranking (fit-to-model)

**In-model gaps closed this run:**
- **Round icon variant** — every competitor emits `ic_launcher_round`. Added `round=true`
  → circular alpha-masked PNG per bucket (anti-aliased edge). Pure image-crate masking, no
  new dep.
- **Correct resource-tree layout** — output mirrors `res/mipmap-<density>/`, so the ZIP drops
  straight into an Android project (matches Image Asset Studio's placement).
- **Play Store 512px icon** — included by default, matching Image Asset Studio's "generate
  Google Play Store icon" option.
- **Configurable resource name** with Android-safe sanitization (lowercase `[a-z0-9_]`, leading
  letter, `ic_launcher` fallback).
- **High-quality resampling** — Lanczos3, not nearest/triangle, for clean small icons.

**Out-of-model (NOT built — would need inputs/surfaces gizza doesn't have):**
- **Adaptive icons (API 26+)** require *two* input layers (foreground + background) and emit
  `mipmap-anydpi-v26/*.xml` + separate foreground/background drawables. gizza's image input is a
  single asset (url⊕ref) — a two-layer upload is unsupported, so adaptive icons are out of model.
- **Shape/padding/background-color compositing** (Android Asset Studio's trim/pad/shape options)
  is a richer editor UX than a single resize; deferred.
- **iOS / cross-platform icon sets** — separate platform conventions; out of scope for an
  Android-named tool.
- **Themed (monochrome) icons** (Android 13+) need a separate monochrome layer — same two-input
  limitation as adaptive icons.

No competitor copy, branding, or trademark was reproduced; only the public technical conventions
(density ladder, mipmap folder names, `ic_launcher`/`ic_launcher_round` resource names) are used.

## Verification

- `cargo test --workspace` — 8 tests pass (sanitization, density table, full-bucket layout,
  decoded 192px/512px sizes, round-mask transparency, non-image rejection, schema drift guard).
- `wafer build` — block.wasm validates/instantiates (pure Rust, no missing WASI imports).
- CLI: `gizza tool android-asset-generator url=… [name=…] [round=true]` — verified default
  (6 assets) and `round=true` (11 assets); ZIP layout inspected.
- No page (ZIP output) → no Playwright; generator landing page re-rendered clean.

## Sources

- [Support different pixel densities — Android Developers](https://developer.android.com/training/multiscreen/screendensities)
- [Create app icons (Image Asset Studio) — Android Developers](https://developer.android.com/studio/write/create-app-icons)
- [Android Asset Studio — Launcher icon generator](https://romannurik.github.io/AndroidAssetStudio/icons-launcher.html)
- [Android Icon Size – Launcher and Google Play Store — Tek Eye](https://www.tekeye.uk/android/android-icon-size)
