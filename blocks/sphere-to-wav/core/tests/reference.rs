//! Cross-check against a reference decoder.
//!
//! `fixtures/demo-1024.sph` is a real SPHERE file (1024-byte ASCII header,
//! 8 kHz mono 16-bit **big-endian** PCM, 2000 sample frames);
//! `fixtures/demo-1024-ffmpeg.pcm` is exactly what ffmpeg's own `nistsphere`
//! demuxer produced from it (`ffmpeg -i demo-1024.sph -f s16le out.pcm`).
//! Our converted samples must be byte-identical to that.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use gizza_ai_sphere_to_wav_core::run;

const SPH: &[u8] = include_bytes!("fixtures/demo-1024.sph");
const FFMPEG_PCM: &[u8] = include_bytes!("fixtures/demo-1024-ffmpeg.pcm");

fn convert(output: &str, encoding: &str, container: &str) -> String {
    run(
        &B64.encode(SPH),
        "base64",
        output,
        encoding,
        "all",
        container,
        "auto",
        0,
        0,
    )
    .expect("conversion succeeds")
}

#[test]
fn raw_samples_match_ffmpegs_nistsphere_demuxer() {
    let raw = B64
        .decode(convert("base64", "pcm16", "raw"))
        .expect("base64 output decodes");
    assert_eq!(raw.len(), FFMPEG_PCM.len());
    assert_eq!(raw, FFMPEG_PCM, "byte-swapped samples match ffmpeg's");
}

#[test]
fn wav_container_is_a_canonical_44_byte_pcm_header() {
    let wav = B64
        .decode(convert("base64", "pcm16", "wav"))
        .expect("base64 output decodes");
    assert_eq!(wav.len(), 44 + FFMPEG_PCM.len());
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[12..16], b"fmt ");
    assert_eq!(&wav[36..40], b"data");
    // RIFF size = file size - 8; data size = the samples.
    assert_eq!(
        u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize,
        wav.len() - 8
    );
    assert_eq!(
        u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize,
        FFMPEG_PCM.len()
    );
    assert_eq!(&wav[44..], FFMPEG_PCM);
}

#[test]
fn info_describes_the_real_corpus_file() {
    let report = convert("info", "pcm16", "wav");
    assert!(report.contains("header_bytes     1024"), "{report}");
    assert!(report.contains("database_id      DEMO (-s4)"), "{report}");
    assert!(report.contains("sample rate      8000 Hz"), "{report}");
    assert!(
        report.contains("byte order       big-endian (sample_byte_format 10)"),
        "{report}"
    );
    assert!(report.contains("sample frames    2000 (0.2500 s)"), "{report}");
    assert!(report.contains("size             3.9 KB (4044 bytes)"), "{report}");
}
