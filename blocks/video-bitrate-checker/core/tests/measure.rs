//! End-to-end measurement against two real muxed files — one MP4 (H.264 + AAC)
//! and one WebM (VP8 + Vorbis). Both are ~10-24 KB clips committed alongside
//! this test, so the numbers below are fixed ground truth.
//!
//! The MP4's expected per-stream bitrates are what `ffprobe -show_entries
//! stream=bit_rate` reports for the same file (19464 and 64796 bit/s). WebM
//! stores no per-stream bitrate at all — ffprobe answers `N/A` there — so the
//! WebM case is what makes the packet-walk measurement worth having.

use gizza_ai_video_bitrate_checker_core::{analyze, Options, Target, Units};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn mp4_per_stream_bitrates_match_ffprobe() {
    let r = analyze(fixture("tiny-128x128-audio.mp4"), &Options::default()).unwrap();

    assert_eq!(r.container, "MP4 / MOV / M4A (ISO BMFF)");
    assert_eq!(r.file_bytes, 23593);
    // `mvhd` play time, which is what ffprobe's format duration reports too.
    assert_eq!(r.duration_seconds, Some(2.0));
    assert_eq!(r.stream_count, 2);
    // 23593 bytes x 8 / 2 s = 94.372 kbit/s, as ffprobe's format bit_rate says.
    assert_eq!(r.overall_bitrate_kbps, Some(94.4));
    assert_eq!(r.overall_bitrate_mbps, Some(0.094));

    let v = &r.streams[0];
    assert_eq!(v.kind, "video");
    assert_eq!(v.codec, "H.264 / AVC");
    assert_eq!((v.width, v.height), (Some(128), Some(128)));
    assert_eq!(v.frame_rate, Some(10.0));
    // ffprobe: 19464 bit/s.
    assert_eq!(v.bitrate_kbps, Some(19.5));

    let a = &r.streams[1];
    assert_eq!(a.kind, "audio");
    assert_eq!(a.codec, "AAC");
    assert_eq!(a.sample_rate, Some(48000));
    assert_eq!(a.channels, Some(1));
    // ffprobe: 64796 bit/s.
    assert_eq!(a.bitrate_kbps, Some(64.8));

    assert_eq!(r.video_bitrate_kbps, Some(19.5));
    assert_eq!(r.audio_bitrate_kbps, Some(64.8));
    // The rest of the file is moov/index/padding, not stream payload.
    assert_eq!(r.container_overhead_kbps, Some(10.1));
    assert_eq!(r.status, "INFO");
}

#[test]
fn webm_streams_are_measured_even_though_the_container_stores_no_bitrate() {
    let r = analyze(fixture("clip-1s.webm"), &Options::default()).unwrap();

    assert_eq!(r.container, "Matroska / WebM");
    assert_eq!(r.stream_count, 2);
    let v = &r.streams[0];
    assert_eq!(v.kind, "video");
    assert_eq!(v.codec, "VP8");
    assert_eq!((v.width, v.height), (Some(64), Some(64)));
    let a = &r.streams[1];
    assert_eq!(a.kind, "audio");
    assert_eq!(a.codec, "Vorbis");
    assert_eq!(a.sample_rate, Some(44100));
    assert_eq!(a.channels, Some(1));

    // Both streams carry real payload and therefore a real measured bitrate.
    assert!(v.bitrate_kbps.unwrap() > 0.0);
    assert!(a.bitrate_kbps.unwrap() > 0.0);
    // 9693 bytes x 8 / 1.003 s.
    assert_eq!(r.overall_bitrate_kbps, Some(77.3));
}

#[test]
fn a_video_ceiling_flags_the_mp4_and_a_wider_one_passes_it() {
    let over = Options {
        max_bitrate: 10.0,
        target: Target::Video,
        ..Default::default()
    };
    let r = analyze(fixture("tiny-128x128-audio.mp4"), &over).unwrap();
    assert_eq!(r.status, "FAIL");
    assert_eq!(r.pass, Some(false));
    assert_eq!(r.reason, "too_high");
    assert_eq!(r.checked_bitrate_kbps, Some(19.5));
    assert!(r.summary.contains("Video bitrate"), "{}", r.summary);

    let under = Options {
        min_bitrate: 10.0,
        max_bitrate: 100.0,
        target: Target::Video,
        ..Default::default()
    };
    let r = analyze(fixture("tiny-128x128-audio.mp4"), &under).unwrap();
    assert_eq!(r.status, "PASS");
    assert_eq!(r.reason, "ok");
}

#[test]
fn an_audio_floor_in_mbps_converts_before_comparing() {
    // 0.128 Mbit/s = 128 kbit/s; the AAC track measures 64.8, so this fails.
    let opts = Options {
        min_bitrate: 0.128,
        units: Units::Mbps,
        target: Target::Audio,
        ..Default::default()
    };
    let r = analyze(fixture("tiny-128x128-audio.mp4"), &opts).unwrap();
    assert_eq!(r.status, "FAIL");
    assert_eq!(r.reason, "too_low");
    assert_eq!(r.units, "Mbps");
    assert_eq!(r.min_bitrate_kbps, Some(128.0));
    assert_eq!(r.checked_bitrate_kbps, Some(64.8));
}

#[test]
fn a_truncated_file_is_rejected_with_a_readable_error() {
    // Half an MP4: ffmpeg puts `moov` at the end unless faststart is used, so a
    // partial download has no track table to read. That must come back as a
    // message a user can act on, not as a panic.
    let full = fixture("tiny-128x128-audio.mp4");
    let half = full[..full.len() / 2].to_vec();
    let e = analyze(half, &Options::default()).unwrap_err();
    assert!(
        e.contains("unrecognised or unsupported media container"),
        "{e}"
    );
}
