//! Decode-path coverage for every advertised compressed input format
//! (flac / ogg-Vorbis / m4a-AAC — wav and mp3 are covered by the unit tests
//! and the CLI matrix). Fixtures are tiny 3.5 s sine tones at different
//! levels, committed under tests/fixtures/, so the test needs no ffmpeg and
//! no network.

use std::io::{Cursor, Read};

use gizza_ai_loudness_matched_ab_prep_core::{
    decode_audio, matched_ab_prep, measure, BitDepth, Mode,
};

const FLAC: &[u8] = include_bytes!("fixtures/tone.flac");
const OGG: &[u8] = include_bytes!("fixtures/tone.ogg");
const M4A: &[u8] = include_bytes!("fixtures/tone.m4a");

fn unzip_entry(zip_bytes: &[u8], name: &str) -> Vec<u8> {
    let mut ar = zip::ZipArchive::new(Cursor::new(zip_bytes)).unwrap();
    let mut f = ar.by_name(name).unwrap();
    let mut v = Vec::new();
    f.read_to_end(&mut v).unwrap();
    v
}

fn lufs_of(wav: Vec<u8>) -> f64 {
    let d = decode_audio("test", wav).unwrap();
    measure("test", &d).unwrap().lufs
}

#[test]
fn flac_and_ogg_inputs_decode_and_match() {
    let m = matched_ab_prep(
        FLAC.to_vec(),
        "tone.flac",
        OGG.to_vec(),
        "tone.ogg",
        Mode::Quieter,
        -16.0,
        BitDepth::Int16,
    )
    .unwrap();
    // The flac fixture is louder than the ogg one, so A gets turned down.
    assert!(m.gain_a_db < 0.0, "flac (louder) turned down: {}", m.gain_a_db);
    assert_eq!(m.gain_b_db, 0.0);
    let la = lufs_of(unzip_entry(&m.zip, "a-matched.wav"));
    let lb = lufs_of(unzip_entry(&m.zip, "b-matched.wav"));
    assert!((la - lb).abs() < 0.1, "matched: {la} vs {lb}");
}

#[test]
fn m4a_input_decodes_and_matches() {
    let m = matched_ab_prep(
        M4A.to_vec(),
        "tone.m4a",
        FLAC.to_vec(),
        "tone.flac",
        Mode::Quieter,
        -16.0,
        BitDepth::Int16,
    )
    .unwrap();
    let la = lufs_of(unzip_entry(&m.zip, "a-matched.wav"));
    let lb = lufs_of(unzip_entry(&m.zip, "b-matched.wav"));
    assert!((la - lb).abs() < 0.1, "matched: {la} vs {lb}");
}
