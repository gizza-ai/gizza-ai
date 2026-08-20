use gizza_ai_flac_picture_extractor_core::{parse, select};

fn fixture(name: &str) -> Vec<u8> {
    let p = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&p).unwrap()
}

#[test]
fn committed_front_cover_fixture_extracts_exact_png_metadata() {
    let report = parse(&fixture("front-cover.flac")).unwrap();
    let picture = select(&report, "front-cover", 1).unwrap();
    assert_eq!(picture.picture_type, 3);
    assert_eq!(picture.picture_type_name, "Cover (front)");
    assert_eq!(picture.mime, "image/png");
    assert_eq!(picture.description, "Front cover art");
    assert_eq!(picture.detected_format, Some("PNG"));
    assert_eq!((picture.width(), picture.height()), (1, 1));
    assert_eq!(picture.data.len(), 69);
    assert_eq!(picture.filename(), "front-cover.png");
}

#[test]
fn committed_no_picture_fixture_reports_the_available_blocks() {
    let report = parse(&fixture("no-picture.flac")).unwrap();
    let err = select(&report, "any", 1).unwrap_err();
    assert!(err.contains("no embedded artwork"), "{err}");
    assert!(err.contains("STREAMINFO"), "{err}");
    assert!(err.contains("PADDING"), "{err}");
}
