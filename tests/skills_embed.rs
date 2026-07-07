//! Every core skill block must be compiled into `SKILLS` with its manifest.
//!
//! Since the lazy-load refactor (see `build.rs` / `src/lazy_skill.rs`), `SKILLS`
//! holds `(slug, manifest.json)` — the block wasm is no longer embedded, it is
//! fetched lazily from `/blocks/<slug>.wasm` on first use. So this asserts the
//! manifest JSON is embedded, not the (no-longer-embedded) wasm bytes.

/// Core skills that must always be built into the app. A missing entry means the
/// block wasn't built before compiling (`blocks/<slug>/target/block.wasm` +
/// `manifest.json` must both exist for `build.rs` to include it).
const EXPECTED_SKILLS: &[&str] = &[
    "clock",
    "web-fetch",
    "xlsx-to-csv",
    "css-select-extract",
    "ffmpeg",
    "image-fetch",
    "imagine",
    "image-resize",
    "image-crop",
    "image-convert",
    "image-compress",
    "video-frame-extract",
    "video-transcode",
    "video-trim",
    "video-compress",
    "video-silence-cut",
];

#[test]
fn core_skills_are_embedded_with_manifests() {
    for &slug in EXPECTED_SKILLS {
        let (_, manifest) = gizza_ai::skills::SKILLS
            .iter()
            .find(|(name, _)| *name == slug)
            .unwrap_or_else(|| {
                panic!("{slug} should be embedded in SKILLS — did you forget to build the block first?")
            });
        assert!(!manifest.is_empty(), "{slug} manifest.json should be non-empty");
        // The second tuple element is the block's manifest.json (a JSON object),
        // not the wasm — the wasm is fetched lazily from /blocks/<slug>.wasm.
        assert!(
            manifest.trim_start().starts_with('{'),
            "{slug} second SKILLS element should be manifest.json (wasm is fetched lazily, not embedded)"
        );
    }
}
