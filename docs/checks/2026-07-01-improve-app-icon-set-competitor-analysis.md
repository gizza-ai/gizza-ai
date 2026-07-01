# app-icon-set competitor analysis (2026-07-01)

## Scope

Tool: `app-icon-set`

Goal: generate a downloadable ZIP containing app icons for iOS, Android, and PWA/web from one source image, using pure Rust so the chat and CLI surfaces can run locally.

## Competitors reviewed

1. Progressier — PWA Icons & iOS Splash Screens Generator
   - Generates PWA icons and iOS assets from an uploaded image.
   - In-model gap: include PWA manifest-ready icons and a maskable icon.
   - Shipped: 192/512 web icons, 512 maskable safe-zone icon, 180 apple-touch-icon, and `manifest.webmanifest`.

2. AppIconGenerator.org
   - Emphasizes full iOS AppIcon.appiconset, Android mipmap folders, Play Store 512 icon, ICO/PWA features.
   - In-model gap: output folders should be drop-in for native projects.
   - Shipped: iOS `AppIcon.appiconset/Contents.json`, Android `res/mipmap-*`, and Google Play 512px icon.

3. AppIcon.co
   - Browser-based icon generation for iOS and Android with no install.
   - In-model gap: privacy-friendly local generation and simple one-image workflow.
   - Shipped: tool resolves a source image and builds the ZIP locally in the block.

4. PWA icon generator CLI tools (for example `pwa-icons`)
   - Generate web/PWA sizes and apple-touch icons from a local input image.
   - In-model gap: CLI/chat downloadable archive rather than project-specific file writes.
   - Shipped: generated assets are returned as one ZIP envelope suitable for CLI `--out`.

5. Figma app-icon maker plugins
   - Export icon assets for many target platforms from a design canvas.
   - Out-of-model gap: integrated design-tool editing, vector export, and platform-specific adaptive layers.
   - Shipped alternative: deterministic raster resizing from a single image, plus background flattening/padding options.

## In-model improvements shipped

- iOS app icon catalog with Xcode `Contents.json` and a flattened opaque 1024px marketing icon.
- Android launcher icons in mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi plus Play Store 512px icon.
- PWA/web icons, padded maskable icon, apple-touch-icon, and manifest.webmanifest.
- Platform toggles (`ios`, `android`, `web`), PWA `app_name`, and hex `background` option.
- ZIP output via media envelope for chat/CLI download.
- Unit tests cover layout, manifest, image dimensions, opacity, maskable padding, and platform toggles.

## Out-of-model / not built

- Standalone page: the current page generator does not model ZIP-of-many-files output from an image input cleanly, so this follows existing no-page file-input/media-envelope patterns.
- SVG/vector source handling beyond what the Rust `image` crate can decode.
- Android adaptive foreground/background layer generation.
- iOS splash screens, macOS/watchOS icon matrices, or favicon `.ico` output.

## Verification notes

The final verification matrix includes block cargo tests, wafer build, CLI install, generator, and CLI smoke with a public image URL writing the generated ZIP. Playwright page coverage is not applicable because this is an image-input ZIP-output tool with no standalone page.
