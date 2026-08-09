# blurhash competitor analysis (2026-08-09)

## Scope

Candidate: `blurhash` — generate compact BlurHash/ThumbHash placeholder strings and previews for progressive image loading.

This is an image-input chat/CLI tool in the current gizza model, matching existing no-page image tools such as `image-info`: it accepts image bytes via `url` or `ref`, returns JSON, and runs in the chat/CLI wasm block. The generic pure-tool page generator does not currently provide a standalone uploaded-image page surface for this shape, so no page spec was added.

## Sources checked

- Wolt BlurHash reference site (`blurha.sh`): explains the compact 20-30 character placeholder model, backend generation, client rendering, and the need to keep dimensions/aspect ratio outside the BlurHash string.
- ToolBelt Blurhash + ThumbHash generator search result: combines both BlurHash and ThumbHash in one image placeholder generator.
- UD5 image-to-BlurHash generator: upload image, return BlurHash string and a tiny preview, pure browser-side flow.
- utilrepo BlurHash generator: positioned around Next.js/SSR/lazy-loading placeholders and roughly 30-byte hashes.
- Giga.tools BlurHash generator search result: browser-based image-to-BlurHash with preview.

`web_extract` was unavailable in this environment, so I used the configured web search results plus direct CLI HTTP fetches for reachable pages and metadata. The analysis paraphrases functionality only.

## Table-stakes capability matrix

| Capability / UX pattern | Competitor expectation | In gizza model? | Decision for this tool |
| --- | --- | --- | --- |
| Image input | Upload or paste/provide an image and generate a placeholder from its pixels. | Yes, via existing `Input::Image` URL/ref pipeline. | Implemented as `url` or `ref` image source using `resolve_source`. |
| BlurHash output | Return the base83 BlurHash string for storage next to the image. | Yes. | Implemented as default `algorithm=blurhash`; returns `hash`, `hash_length`, `hash_bytes`. |
| ThumbHash output | Some modern tools generate ThumbHash as an alternative compact placeholder. | Yes; pure Rust crate is wasm-safe. | Implemented as `algorithm=thumbhash`; returns base64 `hash`, hex `hash_hex`, byte count, aspect ratio-aware preview. |
| Preview rendering | Show or return the decoded blurry placeholder preview. | Yes in JSON; no standalone upload page. | Implemented as PNG `preview_data_url` and CSS `background-image` rule. |
| Detail controls | BlurHash implementations commonly expose x/y component counts or fixed quality presets. | Yes. | Implemented `components_x` and `components_y` integer params, 1-9, default 4x3. |
| Contrast/punch preview tuning | Decoder examples often expose punch/contrast for preview display. | Yes. | Implemented `punch` number param, 0.1-10, default 1.0, affects preview only. |
| Preview size | Tools show a small preview thumbnail; consumers may need a specific long edge. | Yes. | Implemented `preview_size` integer param, 8-512, default 64. |
| Average color fallback | Placeholder workflows often use a one-color fallback while decoding/rendering. | Yes. | Returns `average_color` as hex/rgb/components and `is_dark` flag. |
| Aspect ratio warning | BlurHash does not encode dimensions; consumers must store width/height/aspect ratio separately. | Yes. | Returns `width`, `height`, and rounded `aspect_ratio`; description calls out the BlurHash limitation. |
| Client code snippets | Competitors show framework/client rendering snippets. | Mostly out-of-model for chat block. | Return a portable CSS background-image snippet; framework-specific snippets are documented as out-of-model. |
| Batch image processing | Some production workflows generate placeholders for many images. | Out-of-model for one block invocation. | Not implemented; use repeated CLI calls or future batch wrapper. |
| Visual web upload UI | Competitors commonly provide drag/drop upload and live preview. | Out-of-model for current generic pure image-input page. | Not implemented; current verified surfaces are chat/CLI only. |

## Defaults selected

- `algorithm=blurhash`: widest ecosystem support and matches the backlog name.
- `components_x=4`, `components_y=3`: common landscape default with a 28-character BlurHash.
- `punch=1.0`: reference decoder default contrast.
- `preview_size=64`: small enough for JSON output, large enough to inspect.
- Decode/input cap: image bytes are resolved through the shared image-source guard; core also rejects images whose decoded RGBA budget would exceed the wasm sandbox.

## Worked examples to verify

1. BlurHash from a PNG/JPEG URL:
   `gizza tool blurhash url=https://raw.githubusercontent.com/woltapp/blurhash/master/TypeScript/test/fixtures/img1.png algorithm=blurhash components_x=4 components_y=3 preview_size=32`

2. ThumbHash from the same image:
   `gizza tool blurhash url=https://raw.githubusercontent.com/woltapp/blurhash/master/TypeScript/test/fixtures/img1.png algorithm=thumbhash preview_size=32`

Expected properties: both return `hash`, dimensions, average color, preview data URL, and CSS; BlurHash includes component fields while ThumbHash includes `hash_hex`.

## Out-of-model / not built

- A standalone drag/drop browser upload page for this pure image tool; the current generator page model does not pass uploaded image bytes to arbitrary pure wasm blocks.
- Framework-specific React/Next/Swift/Kotlin code snippets. The tool returns generic hash metadata plus a CSS snippet instead.
- Batch directory crawling, CDN integration, or automatic image upload/storage.
