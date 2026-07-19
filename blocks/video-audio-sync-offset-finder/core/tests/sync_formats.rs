//! Container/codec integration tests on tiny committed fixtures (no ffmpeg, no
//! network at test time). All fixtures carry cuts of the SAME deterministic
//! noise master (44.1 kHz, aperiodic amplitude envelope; see the repo's
//! competitor-analysis doc for the generation recipe):
//!
//! - `a.mp4`   — master[0.0 .. 10.0] s, AAC-LC mono + tiny H.264 track
//! - `b.mp4`   — master[2.5 ..  8.5] s, AAC-LC mono + tiny H.264 track
//! - `c.wav`   — master[1.0 ..  9.0] s, PCM 22.05 kHz (second native rate)
//! - `d.webm`  — master[4.0 ..  9.0] s, Vorbis mono + tiny VP8 track
//! - `e-video-only.mp4` — 3 s H.264, NO audio track
//!
//! Expected offsets follow the sign convention: offset > 0 ⇒ file 2 starts
//! that many seconds AFTER file 1. Same-codec pairs align to ~1 ms; pairs that
//! cross codecs (AAC vs PCM vs Vorbis) can differ by the codecs' priming/edit
//! handling, so their tolerance is wider (measured ≪ the bound in practice).

use gizza_ai_video_audio_sync_offset_finder_core::{confidence_label, sync_offset};

fn fixture(name: &str) -> Vec<u8> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

#[test]
fn mp4_pair_same_codec_aligns_to_millisecond() {
    let r = sync_offset(
        fixture("a.mp4"),
        "file 1 (a.mp4)",
        fixture("b.mp4"),
        "file 2 (b.mp4)",
        120.0,
        0.0,
    )
    .expect("mp4 pair");
    let al = &r.alignment;
    assert!(
        (al.offset_seconds - 2.5).abs() < 0.03,
        "offset {} ≠ +2.5",
        al.offset_seconds
    );
    assert_eq!(al.method, "waveform");
    assert_eq!(
        confidence_label(al.score, true),
        "high",
        "score {}",
        al.score
    );
    assert!(al.correlation > 0.8, "correlation {}", al.correlation);
    assert_eq!(r.a.sample_rate, 44100);
    assert_eq!(r.a.channels, 1);
    assert!(!r.a.truncated && !r.b.truncated);
    assert!(
        (r.a.analyzed_seconds - 10.0).abs() < 0.2,
        "{}",
        r.a.analyzed_seconds
    );
    assert!(
        (r.b.analyzed_seconds - 6.0).abs() < 0.2,
        "{}",
        r.b.analyzed_seconds
    );
}

#[test]
fn swapped_pair_negates_the_offset() {
    let r = sync_offset(
        fixture("b.mp4"),
        "file 1 (b.mp4)",
        fixture("a.mp4"),
        "file 2 (a.mp4)",
        120.0,
        0.0,
    )
    .expect("swapped pair");
    assert!(
        (r.alignment.offset_seconds + 2.5).abs() < 0.03,
        "offset {} ≠ -2.5",
        r.alignment.offset_seconds
    );
}

#[test]
fn mp4_vs_wav_cross_codec_and_rate() {
    let r = sync_offset(
        fixture("a.mp4"),
        "file 1 (a.mp4)",
        fixture("c.wav"),
        "file 2 (c.wav)",
        120.0,
        0.0,
    )
    .expect("mp4 vs wav");
    let al = &r.alignment;
    assert!(
        (al.offset_seconds - 1.0).abs() < 0.06,
        "offset {} ≠ +1.0",
        al.offset_seconds
    );
    assert_eq!(al.method, "waveform");
    assert_eq!(
        confidence_label(al.score, true),
        "high",
        "score {}",
        al.score
    );
    assert_eq!(r.b.sample_rate, 22050, "wav native rate");
}

#[test]
fn mp4_vs_webm_vorbis() {
    let r = sync_offset(
        fixture("a.mp4"),
        "file 1 (a.mp4)",
        fixture("d.webm"),
        "file 2 (d.webm)",
        120.0,
        0.0,
    )
    .expect("mp4 vs webm");
    let al = &r.alignment;
    assert!(
        (al.offset_seconds - 4.0).abs() < 0.08,
        "offset {} ≠ +4.0",
        al.offset_seconds
    );
    assert_eq!(al.method, "waveform");
    // Only ~4 s of aligned overlap (d.webm is 5 s long) — the standard score
    // is honest about the short window; the waveform lock still nails the
    // offset. Measured ≈ 6.7.
    let label = confidence_label(al.score, true);
    assert!(
        label == "high" || label == "medium",
        "label {label}, score {}",
        al.score
    );
    assert!(al.score > 5.0, "score {}", al.score);
}

#[test]
fn max_offset_within_range_still_finds_the_true_peak() {
    // True offset +2.5; max_offset=5 keeps it reachable.
    let r = sync_offset(
        fixture("a.mp4"),
        "file 1 (a.mp4)",
        fixture("b.mp4"),
        "file 2 (b.mp4)",
        120.0,
        5.0,
    )
    .expect("bounded search");
    assert!(
        (r.alignment.offset_seconds - 2.5).abs() < 0.03,
        "offset {} ≠ +2.5",
        r.alignment.offset_seconds
    );
}

#[test]
fn video_only_mp4_is_rejected_with_a_clear_error() {
    let err = sync_offset(
        fixture("a.mp4"),
        "file 1 (a.mp4)",
        fixture("e-video-only.mp4"),
        "file 2 (e-video-only.mp4)",
        120.0,
        0.0,
    )
    .unwrap_err();
    assert!(
        err.contains("file 2 (e-video-only.mp4)") && err.contains("no decodable audio track"),
        "{err}"
    );
}
