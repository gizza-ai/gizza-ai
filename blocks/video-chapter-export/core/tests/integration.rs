//! End-to-end parse+render checks against real fixtures produced by ffmpeg
//! (`-map_chapters`): an MP4 (Nero `chpl`), a Matroska (EBML Chapters), and an
//! M4A — plus a chapterless MP4. See `tests/fixtures/`.

use gizza_ai_video_chapter_export_core::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("tests/fixtures/{name}")).expect("read fixture")
}

const EXPECT_TEXT: &str = "0:00 Intro\n0:10 Main Part\n0:20 Outro & Credits";
const EXPECT_CSV: &str = "index,start,end,start_seconds,end_seconds,title\n\
1,00:00:00.000,00:00:10.000,0.000,10.000,Intro\n\
2,00:00:10.000,00:00:20.000,10.000,20.000,Main Part\n\
3,00:00:20.000,00:00:30.000,20.000,30.000,Outro & Credits";

#[test]
fn mp4_chpl_chapters() {
    let (container, chapters) = parse(&fixture("chapters.mp4")).unwrap();
    assert_eq!(container, "mp4");
    assert_eq!(chapters.len(), 3);
    assert_eq!(chapters[0].title, "Intro");
    assert_eq!(chapters[0].start_ms, 0);
    assert_eq!(chapters[2].title, "Outro & Credits");
    assert_eq!(chapters[2].start_ms, 20_000);
    // Final chapter closed by the mvhd movie duration.
    assert_eq!(chapters[2].end_ms, Some(30_000));
}

#[test]
fn matroska_ebml_chapters() {
    let (container, chapters) = parse(&fixture("chapters.mkv")).unwrap();
    assert_eq!(container, "matroska");
    assert_eq!(chapters.len(), 3);
    assert_eq!(chapters[1].title, "Main Part");
    assert_eq!(chapters[1].start_ms, 10_000);
    assert_eq!(chapters[1].end_ms, Some(20_000));
}

#[test]
fn m4a_audio_chapters() {
    let (container, chapters) = parse(&fixture("chapters.m4a")).unwrap();
    assert_eq!(container, "mp4");
    assert_eq!(chapters.len(), 3);
}

#[test]
fn mp4_and_mkv_render_identically() {
    // Same logical chapters in two containers -> byte-identical exports.
    for f in ["chapters.mp4", "chapters.mkv", "chapters.m4a"] {
        let bytes = fixture(f);
        assert_eq!(export_chapters(&bytes, "text", f).unwrap().content, EXPECT_TEXT, "{f} text");
        assert_eq!(export_chapters(&bytes, "csv", f).unwrap().content, EXPECT_CSV, "{f} csv");
    }
}

#[test]
fn json_export_is_valid_json_array() {
    let out = export_chapters(&fixture("chapters.mkv"), "json", "x.mkv").unwrap();
    assert_eq!(out.chapter_count, 3);
    let v: serde_json::Value = serde_json::from_str(&out.content).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 3);
    assert_eq!(v[0]["title"], "Intro");
    assert_eq!(v[2]["start_ms"], 20_000);
}

#[test]
fn cue_export_has_tracks() {
    let out = export_chapters(&fixture("chapters.mp4"), "cue", "movie.mp4").unwrap();
    assert!(out.content.contains("FILE \"movie.mp4\" WAVE"));
    assert!(out.content.contains("  TRACK 01 AUDIO"));
    assert!(out.content.contains("    INDEX 01 00:20:00"));
}

#[test]
fn no_chapters_returns_empty_not_error() {
    let out = export_chapters(&fixture("no-chapters.mp4"), "json", "n.mp4").unwrap();
    assert_eq!(out.chapter_count, 0);
    assert_eq!(out.content, "[]");
}
