# splash-screen-generator — competitor analysis (2026-08-29)

Scan run **before** implementing, per `/create-next-tool` step 4. One web search
("splash screen generator iOS Android launch screen all device resolutions from logo"),
then the top real tools were skimmed. Everything below is **paraphrased** — no competitor
copy, branding, or trademarks are reproduced or reused.

## Competitors skimmed

| # | Tool | Shape | Notes |
|---|------|-------|-------|
| 1 | Progressier PWA icons & iOS splash generator | hosted web app | Upload one image → full iOS launch-image set + the HTML `<head>` meta tags, delivered as a bundle. Accepts PNG/JPG/SVG/WEBP; recommends ≥512×512 at 1:1. Covers iPhone SE → current iPhone/iPad families, portrait **and** landscape. |
| 2 | TestMu AI splash screen generator | hosted web app | iOS + Android + PWA in one click, ZIP download. Explicit controls: background colour as HEX/RGB/HSL, a **logo-size slider**, rounded-corner toggle, orientation choice (portrait / landscape / both), optional app name + version. Android coverage described as "all density buckets mdpi → xxxhdpi". |
| 3 | `pwa-asset-generator` (npm CLI, the CLI-side reference) | local CLI | Flags: `--background` (default `transparent`), `--padding` (default `10%`), `--opaque` (default true), `--splash-only` / `--icon-only`, `--portrait-only` / `--landscape-only`, `--dark-mode` (default false, emits `prefers-color-scheme: dark` launch images), `--type` png\|jpg (default jpg), `--quality` (default 70), `--path`, `--index`/`--manifest` auto-injection, `--scrape` for live Apple device specs. Emits `apple-touch-startup-image` `<link>` tags with `device-width` / `device-height` / `-webkit-device-pixel-ratio` / `orientation` media queries. |
| 4 (extra) | NextNative free icon + splash generator | hosted web app | ~40 variants in one ZIP, separate iOS/Android folders, a README with placement instructions, background-colour presets, rounded-corner toggle, live preview, 10 MB upload cap, recommends ≥1024×1024. |

Appscope's splash-screen page (a 5th candidate) returned HTTP 503 on two fetches, so it was
dropped rather than guessed at; the four above are enough to fix the table stakes.

## Table stakes → where each one landed

| Table stake (seen on ≥2 competitors) | Verdict | Where |
|---|---|---|
| One logo in → every common device resolution out, as a single ZIP | **in-model** | Core `generate_zip`; ZIP envelope like `app-icon-set` |
| Background colour, hex, with `#` optional | **in-model** | `background` param, default `#ffffff`; `#rgb`/`#rrggbb`/`#rrggbbaa` all parse |
| Logo size / padding control | **in-model** | `logo_scale` (0.05–0.9, default 0.4 = logo's long edge is 40 % of the canvas's short side); equivalent to the competitors' padding %/slider |
| Portrait **and** landscape | **in-model** | `orientation` = `portrait` (default) \| `landscape` \| `both` |
| Platform scoping (iOS / Android on-off) | **in-model** | `ios` / `android` booleans, both default true |
| iOS device coverage incl. current iPhone/iPad | **in-model** | 18 distinct portrait resolutions, iPhone SE (1st gen) → iPhone 16 Pro Max, iPad 9.7" → iPad Pro 12.9" |
| Android density buckets mdpi → xxxhdpi | **in-model** | `res/drawable-<density>/splash.png` + `-land-` variants |
| `apple-touch-startup-image` meta tags with media queries | **in-model** | `ios/apple-touch-startup-image.html` generated alongside the PNGs |
| Dark-mode variant set | **in-model** | `dark_background` (empty = off); when set, a second `dark/` set plus `prefers-color-scheme: dark` media queries |
| PNG **or** JPEG output + quality knob | **in-model** | `format` = `png` (default) \| `jpeg`, `quality` 1–100 (default 82, JPEG only) |
| README / placement instructions in the bundle | **in-model** | `README.txt` in the ZIP |
| Android 12+ splash **icon** (not a full-bleed bitmap) | **in-model** | `android/res/drawable/splash_icon.png` — 1152 px canvas, logo inside the 768 px (192 dp) safe circle |

## Considered and deliberately not built

| Item | Why |
|---|---|
| SVG logo input | The `image` crate has no SVG rasteriser and adding one (resvg/usvg) is a large wasm dependency; PNG / JPEG / WebP / GIF / BMP are accepted and the error names the supported list. |
| Rounded-corner mask on the logo | Splash logos are rarely masked (that is an app-**icon** convention, and `app-icon-set` already owns icons); it would add a param that most runs leave off. Rejected on schema-bloat grounds. |
| Live preview, drag-and-drop upload, colour-picker widget | Pure UX of a hosted form. This block's output is a ZIP of many files, which fits neither the text nor the single-media page shape, so it ships **chat + CLI, no standalone page** — the same call `app-icon-set`, `favicon-generator` and `android-asset-generator` already make. |
| Scraping Apple's live device list at runtime | Out of model: the block is deterministic and offline apart from fetching the logo. The device table is compiled in and can be updated with the code. |
| Auto-patching the user's `index.html` / `manifest.json` | Out of model: no filesystem access to a user project; the meta-tag snippet is emitted as a file to paste instead. |
| App name / version text rendered onto the splash | Needs font embedding and text layout; `text-banner-image` / `add-text-to-image` already cover rendering text onto an image, and a caller can chain them. |
| Framework-specific naming (React Native / Flutter presets) | Every framework wants a different tree; the emitted Xcode-friendly `ios/` + Android `res/` layout plus the README covers the two that have a documented standard. |

## Design decisions recorded

- **Pure Rust, not ffmpeg.** The backlog row tagged this `ffmpeg`, but ffmpeg cannot emit a
  multi-file archive and cannot run in the chat Service Worker. `image` + `zip` (both already
  proven wasm-safe here) composite the logo over the background and encode every size, so the
  block runs on **all** backends.
- **Render cache keyed by (width, height).** Several devices share a resolution and the iOS
  landscape set is the transpose of the portrait set, so each distinct canvas is composited and
  encoded once and the bytes are written into the ZIP under every name that needs them.
- **Default `orientation = portrait`.** Portrait is what a launch screen is for in practice and
  it halves the work of a default run; `both` is one word away.
- **Caps stated up front:** 16 MiB source image, 64 MiB generated archive, and the error
  messages name the limit that was hit.
