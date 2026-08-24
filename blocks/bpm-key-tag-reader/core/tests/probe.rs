use gizza_ai_bpm_key_tag_reader_core::{read, Options};

fn fixture(name: &str) -> Vec<u8> {
    let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&p).unwrap()
}

#[test]
fn reads_id3v2_bpm_key_and_energy_tags() {
    let mut opts = Options::default();
    opts.include_all_tags = true;
    let report = read(&fixture("id3v2-tagged.mp3"), opts).unwrap();
    assert!(report.found);
    assert_eq!(report.container, "MP3 (MPEG audio)");
    let bpm = report.bpm.unwrap();
    assert_eq!(bpm.bpm, 128.0);
    assert_eq!(bpm.source_field, "TBPM");
    let key = report.key.unwrap();
    assert_eq!(key.standard.as_deref(), Some("A minor"));
    assert_eq!(key.camelot.as_deref(), Some("8A"));
    assert_eq!(key.open_key.as_deref(), Some("1m"));
    assert_eq!(key.source_field, "TKEY");
    assert_eq!(report.energy.unwrap().energy, "7");
    assert!(report
        .all_tags
        .unwrap()
        .iter()
        .any(|t| t.field == "TXXX:INITIALKEY"));
}

#[test]
fn reads_vorbis_comment_bpm_key_and_energy_tags() {
    let mut opts = Options::default();
    opts.include_all_tags = true;
    let report = read(&fixture("vorbis-tagged.flac"), opts).unwrap();
    assert!(report.found);
    assert_eq!(report.container, "FLAC (native)");
    let bpm = report.bpm.unwrap();
    assert_eq!(bpm.bpm, 174.5);
    assert_eq!(bpm.bpm_rounded, 175);
    assert_eq!(bpm.source_field, "BPM");
    let key = report.key.unwrap();
    assert_eq!(key.standard.as_deref(), Some("F# minor"));
    assert_eq!(key.camelot.as_deref(), Some("11A"));
    assert_eq!(key.open_key.as_deref(), Some("4m"));
    assert_eq!(key.source_field, "INITIALKEY");
    assert_eq!(report.energy.unwrap().source_field, "ENERGYLEVEL");
}

#[test]
fn untagged_audio_is_a_clean_not_found_report() {
    let report = read(&fixture("untagged.wav"), Options::default()).unwrap();
    assert!(!report.found);
    assert_eq!(report.container, "WAVE (RIFF)");
    assert!(report
        .notes
        .iter()
        .any(|n| n.contains("no tempo or key tags")));
}

#[test]
fn reads_mp4_tmpo_atom_and_itunes_freeform_key_and_energy_tags() {
    let report = read(&fixture("mp4-tagged.m4a"), Options::default()).unwrap();
    assert_eq!(report.container, "MP4 / M4A (ISO BMFF)");
    assert!(report.found);
    let bpm = report.bpm.unwrap();
    assert_eq!(bpm.bpm, 140.0);
    // symphonia recognises the `tmpo` atom, so it maps it onto the standard BPM
    // key and leaves the literal atom name empty — we name it back to `tmpo`.
    assert_eq!(bpm.source_field, "tmpo");
    // The key is stored in Open Key notation, so it exercises that parse path.
    let key = report.key.unwrap();
    assert_eq!(key.raw, "1m");
    assert_eq!(key.standard.as_deref(), Some("A minor"));
    assert_eq!(key.camelot.as_deref(), Some("8A"));
    assert_eq!(key.source_field, "com.apple.iTunes:initialkey");
    let energy = report.energy.unwrap();
    assert_eq!(energy.energy, "6");
    assert_eq!(energy.source_field, "com.apple.iTunes:EnergyLevel");
    assert!(report.notes.is_empty(), "got {:?}", report.notes);
}
