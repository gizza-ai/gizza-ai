//! The exact fixtures used by the page's `[[example]]` chips and by the CLI
//! examples in `page/content.md`. If an example ever stops producing the
//! documented result, this test fails instead of the page quietly shipping a
//! broken "Try:" chip.

use gizza_ai_midi_track_splitter_core::{split, Options, OutputMode, SplitBy};

/// Format 1, 96 PPQ: conductor (120 BPM, 4/4) + "Piano" on channel 1 (2 notes)
/// + "Bass" on channel 2 (1 note).
const THREE_TRACK_HEX: &str = "4d546864000000060001000300604d54726b0000002000ff0309436f6e647563746f7200ff510307a12000ff58040402180800ff2f004d54726b0000002000ff03055069616e6f00c00000903c4060903c00009040466090400000ff2f004d54726b0000001800ff03044261737300c12100912464814091240000ff2f00";
/// The same file as base64, used by the encoding example.
const THREE_TRACK_B64: &str = "TVRoZAAAAAYAAQADAGBNVHJrAAAAIAD/AwlDb25kdWN0b3IA/1EDB6EgAP9YBAQCGAgA/y8ATVRyawAAACAA/wMFUGlhbm8AwAAAkDxAYJA8AACQQEZgkEAAAP8vAE1UcmsAAAAYAP8DBEJhc3MAwSEAkSRkgUCRJAAA/y8A";
/// Format 0, 96 PPQ: ONE track carrying channel 1 (Electric Piano 1) and
/// channel 10 (drums) at 150 BPM.
const FORMAT_0_HEX: &str = "4d546864000000060000000100604d54726b0000002600ff0304536f6e6700ff5103061a8000c0040090435a0099246e609043000099240000ff2f00";

#[test]
fn default_example_splits_the_three_track_file_by_track() {
    let out = split(THREE_TRACK_HEX, "hex", &Options::default()).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.filename.as_str()).collect();
    assert_eq!(names, vec!["part-02-piano.mid", "part-03-bass.mid"]);
    assert_eq!(out.files[0].notes, 2);
    assert_eq!(out.files[1].notes, 1);
    assert_eq!(out.conductor_events, 3);
    assert!(out.summary().starts_with(
        "Split a format-1 file (3 track(s), 96 ticks per quarter note, 3 note(s)) into 2 single-part file(s), one per track."
    ));
}

#[test]
fn base64_and_hex_examples_are_the_same_file() {
    let a = split(THREE_TRACK_HEX, "auto", &Options::default()).unwrap();
    let b = split(THREE_TRACK_B64, "auto", &Options::default()).unwrap();
    assert_eq!(a.files[0].midi, b.files[0].midi);
    assert_eq!(a.files[1].midi, b.files[1].midi);
}

#[test]
fn the_list_example_previews_the_parts_without_bytes() {
    let opts = Options {
        output: OutputMode::List,
        skip_empty: false,
        ..Options::default()
    };
    let out = split(THREE_TRACK_HEX, "hex", &opts).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, vec!["Conductor", "Piano", "Bass"]);
    assert!(out.files.iter().all(|f| f.midi.is_empty()));
}

#[test]
fn the_format_0_example_is_cut_by_channel_and_names_the_drum_part() {
    let out = split(FORMAT_0_HEX, "hex", &Options::default()).unwrap();
    assert!(out.auto_channel);
    assert_eq!(out.split_by, "channel");
    let names: Vec<&str> = out.files.iter().map(|f| f.filename.as_str()).collect();
    assert_eq!(
        names,
        vec!["part-01-electric-piano-1.mid", "part-10-drum-kit.mid"]
    );
    assert_eq!(out.files[1].channels, vec![10]);
    // 150 BPM (400000 us per quarter note) is read from the source file.
    assert!((out.tempo_bpm - 150.0).abs() < 1e-9);
    // One beat at 96 PPQ / 150 BPM = 0.4 s.
    assert!((out.files[0].seconds - 0.4).abs() < 1e-9);
}

#[test]
fn the_channel_example_gathers_channels_across_tracks() {
    let opts = Options {
        split_by: SplitBy::Channel,
        ..Options::default()
    };
    let out = split(THREE_TRACK_HEX, "hex", &opts).unwrap();
    let names: Vec<&str> = out.files.iter().map(|f| f.filename.as_str()).collect();
    // Each source track is devoted to one channel, so the parts keep their
    // track names rather than falling back to the General MIDI instrument.
    assert_eq!(names, vec!["part-01-piano.mid", "part-02-bass.mid"]);
}
