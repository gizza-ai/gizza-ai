# Page / surface patterns (input rendering, defaults, drift, tool shapes)

**Declarative control kinds (2026-07-03 sweep — USE THESE AT BUILD TIME):** beyond the
schema-derived controls, meta.toml `[[input]]` supports `kind = "slider"` (range mirror onto the
canonical number box; live drag mirrors value, ONE change event on release = one run; set `step`),
`kind = "color"` (hybrid: native swatch two-way mirrored onto a hex TEXT field — named colors,
`transparent`, alpha hex, and comma lists stay expressible; the text field is canonical),
`kind = "tag-list"`, and `kind = "date"|"time"|"datetime-local"`. `[input.labels]` maps enum
values → friendly `<select>` labels (values stay canonical). `[[example]]` chips prefill params
in one click — add them whenever competitors ship presets. ffmpeg pages also get paste-to-upload
generically, and `format = "text"` pages get a Download link. Generated CLI examples are
schema-derived and runnable — never hand-write CLI examples in content.md that can drift.

**ffmpeg-page field marshaling (2026-07-03):** checkboxes arrive at `build_argv` as
`"true"`/`"false"` via `readField()` (an old bug sent constant `"on"` — parse positive-truthy and
Playwright-test one NON-default checkbox state). The ffmpeg path numeric-coerces numeric-LOOKING
strings before `build_argv` — `kind = "color"` fields are exempt platform-wide; any other string
param whose values can look numeric (bare hex, digit codes) needs an end-to-end digits-only test.

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

**Audio test fixtures: lavfi `sine` is ~1/8 amplitude, NOT full scale (2026-07-02).** A fixture
made with `sine=frequency=440` has RMS ≈ -47.5 dB after `volume=0.05` (the source itself sits
around -18 dB), so absolute RMS windows computed from "amplitude × gain" are ~18.7 dB off and
fail mysteriously with correct-looking ratios. For gain-type tools (volume, fades), write
Playwright assertions as OUTPUT-RMS ÷ INPUT-RMS ratios (decode both via WebAudio) — immune to
fixture amplitude. Absolute windows are fine only for loudness-normalizing tools (loudnorm
targets absolute output loudness regardless of input).

**Non-standard output extensions (`.m4r` — 2026-07-19):** ffmpeg cannot infer a muxer from an
extension it doesn't know (`Unable to choose an output format for 'out.m4r'`) — pass the muxer
explicitly (`-f ipod` for m4r; the ipod muxer is what `.m4a` already maps to, so it's present in
both native ffmpeg and the page's @ffmpeg/core build). ALSO check the page runtime's `EXT_MIME`
table in `tools/generator/assets/runtime/tool-ffmpeg.js`: an unknown output extension renders
`application/octet-stream`, which silently breaks the `<audio>`/`<video>` preview even though
ffmpeg succeeded — add the mapping (m4r → audio/mp4 is in now) + a `js/tool-ffmpeg.test.js` case,
then regenerate pages so the copied runtime picks it up.

**Multi-input ffmpeg (e.g. video-concat) is effectively un-buildable here:** the page file-input is a
single upload and ffmpeg can't run in the chat SW, so it'd be CLI-only — skiplist + defer.
(Multi-IMAGE pure-Rust tools ARE buildable as chat+CLI, no page — see the gif-from-images entry in
wasm-crates.md.)

**Playwright `page.fill` on a many-line textarea is minutes-slow (2026-07-20, data-anonymizer):**
filling a textarea with ~10k newline-separated rows routes through Chromium `insertText`, which
took 4.5 MINUTES per fill (all wall-clock wait, ~0 CPU) — the spec passed but would drag CI. For
big cap-boundary fixtures set the value directly and dispatch the same event the driver listens
to: `await page.locator('#in-data').evaluate((el, v) => { el.value = v; el.dispatchEvent(new
Event('input', { bubbles: true })); }, big)` — identical trigger path (field listeners bind
"input"), runs in ~100 ms. Normal-sized `page.fill` calls stay as-is.

**ffmpeg `deshake` rx/ry must be a multiple of 16 (2026-07-20, video-stabilize):** the filter
rejects any other radius at graph-init time ("rx must be a multiple of 16", exit 176) — docs say
0–64 but only 16/32/48/64 actually initialize. A strength→radius map must SNAP to those steps
(quartiles), not scale linearly; argv unit tests pass either way, so only a REAL ffmpeg run
catches it (another advertised-values-matrix save). Also: filter-chain expressions like
`scale=trunc(iw*1.068/2)*2:trunc(ih*1.068/2)*2,crop=trunc(iw/1.068/2)*2:trunc(ih/1.068/2)*2`
(zoom-then-recrop to ~original even dims) work in both native ffmpeg and @ffmpeg/core, and give
EXACT output dims to assert in Playwright (128→126 at z=1.068).
