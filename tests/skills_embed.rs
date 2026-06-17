#[test]
fn clock_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/clock"),
        "gizza-ai/clock should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/clock")
        .expect("clock");
    assert!(!bytes.is_empty(), "clock wasm bytes non-empty");
    // Quick wasm magic-number check.
    assert_eq!(&bytes[..4], b"\0asm", "clock bytes look like wasm");
}

#[test]
fn web_fetch_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/web-fetch"),
        "gizza-ai/web-fetch should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/web-fetch")
        .expect("web-fetch");
    assert!(!bytes.is_empty(), "web-fetch wasm bytes non-empty");
    assert_eq!(&bytes[..4], b"\0asm", "web-fetch bytes look like wasm");
}

#[test]
fn ffmpeg_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/ffmpeg"),
        "gizza-ai/ffmpeg should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/ffmpeg")
        .expect("ffmpeg");
    assert!(!bytes.is_empty(), "ffmpeg wasm bytes non-empty");
    assert_eq!(&bytes[..4], b"\0asm", "ffmpeg bytes look like wasm");
}

#[test]
fn image_fetch_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/image-fetch"),
        "gizza-ai/image-fetch should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/image-fetch")
        .expect("image-fetch");
    assert!(!bytes.is_empty(), "image-fetch wasm bytes non-empty");
    assert_eq!(&bytes[..4], b"\0asm", "image-fetch bytes look like wasm");
}

#[test]
fn imagine_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/imagine"),
        "gizza-ai/imagine should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/imagine")
        .expect("imagine");
    assert!(!bytes.is_empty(), "imagine wasm bytes non-empty");
    assert_eq!(&bytes[..4], b"\0asm", "imagine bytes look like wasm");
}

#[test]
fn image_resize_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/image-resize"),
        "gizza-ai/image-resize should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/image-resize")
        .expect("image-resize");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn image_crop_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/image-crop"),
        "gizza-ai/image-crop should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/image-crop")
        .expect("image-crop");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn image_convert_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/image-convert"),
        "gizza-ai/image-convert should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/image-convert")
        .expect("image-convert");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn heic_to_jpg_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/heic-to-jpg"),
        "gizza-ai/heic-to-jpg should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/heic-to-jpg")
        .expect("heic-to-jpg");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn video_frame_extract_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/video-frame-extract"),
        "gizza-ai/video-frame-extract should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/video-frame-extract")
        .expect("video-frame-extract");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn video_transcode_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/video-transcode"),
        "gizza-ai/video-transcode should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/video-transcode")
        .expect("video-transcode");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}

#[test]
fn video_trim_skill_is_embedded() {
    assert!(
        gizza_ai::skills::SKILLS
            .iter()
            .any(|(n, _)| *n == "gizza-ai/video-trim"),
        "gizza-ai/video-trim should be embedded — did you forget to build the block first?"
    );
    let (_, bytes) = gizza_ai::skills::SKILLS
        .iter()
        .find(|(n, _)| *n == "gizza-ai/video-trim")
        .expect("video-trim");
    assert!(!bytes.is_empty());
    assert_eq!(&bytes[..4], b"\0asm");
}
