# Page / surface patterns (input rendering, defaults, drift, tool shapes)

**Page input field types (meta.toml `[[input]]`):** the generator renders each field by the
descriptor's Param type, NOT the meta — so Playwright must match: `Param::enumv` → `<select>`
(`page.selectOption('#in-<name>', value)`); `Param::boolean` → `<input type=checkbox>` (use
`page.check`/`uncheck`, default reflects `.default(true)`); `Param::integer`/`number` → a field
(`page.fill`); `Param::string` → `<input>` UNLESS the meta `[[input]]` sets `multiline = true`,
which renders a `<textarea>`. **Use `multiline = true` for any text/code/key field** so pasted
newlines are preserved (a plain `<input>` strips them — this is why multi-line keys can't go in a
plain field). Pages still need `format = "text"` or output is gated off.

**PAGE BOOLEAN CHECKBOX DEFAULT:** the page generator renders a boolean param as `<input type=checkbox
checked[default]>` — i.e. the box is **checked iff the descriptor `Param::boolean(...).default(true)`**. So a
`default(true)` boolean shows CHECKED on load, and the web `run()` receives `"true"`; unchecked sends
`"false"`. In the web wrapper use **positive truthy** (`matches!(v, "true"|"1"|"on"|"yes")`). In Playwright,
a default-true checkbox is already checked — to test the off-path call `page.uncheck('#in-<name>')` (not just
leaving it).

**DRIFT GOTCHA — number-param defaults serialize as `N.0`:** `Param::number("x").default(1.0)` renders
`"default": 1.0` in the schema. In the authored drift JSON write `"default": 1.0` (NOT `1`) — serde_json
treats `1` as an integer and `1.0` as a float, so they compare unequal and the drift test fails. (Integer
params with `.default(1)` correctly render `1`.)

**Clocks across surfaces:** `std::time::SystemTime::now()` and `chrono::Utc::now()` DO instantiate in the
chat block (wafer provides the clock import) and work natively in the CLI. But the PAGE target
(wasm32-unknown-unknown) has NO std clock — get time in the web crate via `js_sys::Date::now()/1000`
(add `js-sys` to web/Cargo.toml). Make the core take an explicit timestamp so it's deterministic and
each surface supplies its own clock.

**Tool shapes that recur:** SVG/PDF/image-bytes output → `build_media_envelope` (mime
`image/svg+xml` / `application/pdf` / `image/png`), chat+CLI, NO page (image-bytes have no page render
mode); hand-build SVG with `format!`/`r##"..."##` (colours like `#fff` need the `##` raw delimiter).
File-input→JSON (strings/unzip/detect-file-type) use `AssetKind::Any` (there is no `AssetKind::File`)
+ flat `Resp` via `GuestResult::respond`. Text+passphrase/key tools can reuse another block's core via
`path = "../../<other>/core"` (text-encrypt reuses encrypt-file's AES-GCM core).

**Page output formats:** the page driver renders `format = "image" | "video" | "audio"` (media,
with a download link) or anything else as `text`. Audio output (`format = "audio"` → `<audio
controls>`) works — **video→audio** ffmpeg tools are fully supported (set the page `format = "audio"`,
give the block an `audio/*` mime via the core `Format::mime()`, and use `build_media_envelope`;
CLI-test with a video that actually has an audio track — many small test clips are silent and fail
with "nothing to encode").

**Audio-INPUT tools are supported too (since 2026-07-02):** the descriptor takes `Input::Audio`
(url⊕ref chat/CLI schema with the "Audio URL" wording), `resolve_source` takes
`AssetKind::Audio` (accepts the `audio/*` MIME class), and the page file input uses
`[[input]] source="file" accept="audio/*"` with `runtime="ffmpeg"`. Output formats map via
`format_to_mime_and_ext(AssetKind::Audio, "mp3"|"wav"|"ogg"|"flac"|"m4a")`. This unlocked the
plain-ffmpeg audio family (trim-audio, audio-convert, audio-normalize, waveform-image, …) — see
the 2026-07-02 skiplist sweep. Tools needing an ML model (transcribe, stem-split, autotune) stay
skiplisted. CLI-test with a real public audio URL (SSRF guard applies — see ops.md).

**Multi-input ffmpeg (e.g. video-concat) is effectively un-buildable here:** the page file-input is a
single upload and ffmpeg can't run in the chat SW, so it'd be CLI-only — skiplist + defer.
(Multi-IMAGE pure-Rust tools ARE buildable as chat+CLI, no page — see the gif-from-images entry in
wasm-crates.md.)
