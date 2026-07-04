//! Per-format fact table for the programmatic "X to Y" converter pair pages
//! (see `pairs.rs`). One entry per audio/image format the two converter tools
//! can read or write, with the copy fragments the pair pages are assembled
//! from — every fragment is format-specific, so no two pair pages share their
//! prose (the thin-content guard).
//!
//! All claims must stay consistent with the parent tools' documented behavior
//! (`blocks/audio-convert/page/content.md`, `blocks/image-convert/page/content.md`):
//! audio bitrate 32–320 kbps (lossy targets only), album art dropped, 10 MiB
//! input limit; image quality 1–100 (JPEG/WebP only), PNG lossless.

/// Which converter tool a format belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Audio,
    Image,
}

/// Facts and copy fragments for one format.
pub struct FormatInfo {
    /// Canonical key — also the value the parent tool's `?format=` accepts
    /// when the format is a conversion target (e.g. "jpeg", not "jpg").
    pub key: &'static str,
    /// Alternate slug spellings that resolve to this entry (e.g. "jpg").
    pub aliases: &'static [&'static str],
    /// Display name ("MP3", "WebP").
    pub name: &'static str,
    /// Codec/container one-liner for the comparison table.
    pub codec: &'static str,
    pub family: Family,
    /// True when encoding to this format discards audio/image detail.
    pub lossy: bool,
    /// Full-sentence identity blurb — the intro's per-format paragraph.
    pub blurb: &'static str,
    /// Short phrase: what the format is typically used for.
    pub uses: &'static str,
    /// Short phrase: its headline strength (comparison-table cell).
    pub strength: &'static str,
    /// Short phrase: its headline caveat (comparison-table cell).
    pub caveat: &'static str,
    /// Comparison-table cell: typical file size.
    pub size_note: &'static str,
    /// Comparison-table cell: where it plays/opens.
    pub compat: &'static str,
    /// Image-only comparison-table cell: transparency support.
    pub transparency: Option<&'static str>,
    /// Image-only comparison-table cell: animation support.
    pub animation: Option<&'static str>,
}

/// The full fact table: the 8 audio formats and 6 image formats the converter
/// pair pages cover.
pub const FORMATS: &[FormatInfo] = &[
    // ---- audio ----
    FormatInfo {
        key: "mp3",
        aliases: &[],
        name: "MP3",
        codec: "MPEG Layer III audio",
        family: Family::Audio,
        lossy: true,
        blurb: "MP3 is the most widely supported audio format there is — a lossy \
                codec that shrinks audio to a fraction of its uncompressed size \
                and plays on virtually anything with a speaker.",
        uses: "sharing, podcasts and everyday listening",
        strength: "plays everywhere; small files",
        caveat: "lossy — encoding discards some audio detail to save space",
        size_note: "small — about 1.4 MB per minute at 192 kbps",
        compat: "universal — effectively every device and app",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "wav",
        aliases: &[],
        name: "WAV",
        codec: "uncompressed 16-bit PCM in a RIFF container",
        family: Family::Audio,
        lossy: false,
        blurb: "WAV stores raw, uncompressed PCM samples — the audio equivalent \
                of a bitmap. Files are huge, but every editor, DAW and operating \
                system opens them without a second thought.",
        uses: "editing, DAWs and audio production",
        strength: "universal uncompressed PCM — ideal for editing",
        caveat: "huge files for what they hold",
        size_note: "very large — roughly 10 MB per minute of 16-bit stereo",
        compat: "universal — opens in every editor and OS",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "ogg",
        aliases: &[],
        name: "OGG",
        codec: "Vorbis audio in an Ogg container",
        family: Family::Audio,
        lossy: true,
        blurb: "OGG (Vorbis) is a free, open lossy format with very good quality \
                per byte — a favourite in games, open-source software and \
                projects that want to avoid patent-encumbered codecs.",
        uses: "games, open-source pipelines and the web",
        strength: "open and royalty-free; good quality per byte",
        caveat: "less at home in Apple's ecosystem than MP3 or M4A",
        size_note: "small — comparable to MP3 at the same bitrate",
        compat: "broad, though Apple software often needs a third-party player",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "flac",
        aliases: &[],
        name: "FLAC",
        codec: "Free Lossless Audio Codec",
        family: Family::Audio,
        lossy: false,
        blurb: "FLAC compresses audio without losing anything — a perfect, \
                bit-for-bit copy at a fraction of the WAV size, which makes it \
                the default choice for archiving music.",
        uses: "archiving and lossless music libraries",
        strength: "lossless and compressed — a perfect copy, smaller than WAV",
        caveat: "much larger than lossy formats; some older hardware skips it",
        size_note: "medium — typically 50–70% of the equivalent WAV",
        compat: "wide in modern software; patchy on older hardware players",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "m4a",
        aliases: &[],
        name: "M4A",
        codec: "AAC audio in an MPEG-4 container",
        family: Family::Audio,
        lossy: true,
        blurb: "M4A wraps AAC audio in an MPEG-4 container — the format iTunes \
                and Apple Music use. AAC squeezes better quality than MP3 out of \
                the same bitrate, at the cost of slightly narrower support.",
        uses: "Apple devices and small high-quality files",
        strength: "better quality than MP3 at the same bitrate",
        caveat: "slightly less universal than MP3",
        size_note: "small — like MP3, often better quality per byte",
        compat: "excellent on Apple devices; broad elsewhere",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "aac",
        aliases: &[],
        name: "AAC",
        codec: "raw AAC (Advanced Audio Coding) stream",
        family: Family::Audio,
        lossy: true,
        blurb: "AAC is the codec behind M4A files, streaming services and much \
                of broadcast audio; as a bare .aac stream it carries the same \
                lossy audio without the MPEG-4 wrapper — and without proper tag \
                support.",
        uses: "streams, broadcast audio and recorder output",
        strength: "modern lossy codec, efficient at every bitrate",
        caveat: "bare .aac streams carry no metadata and trip up some players",
        size_note: "small — similar to MP3 at the same bitrate",
        compat: "the codec is everywhere, but fewer apps open bare .aac files",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "opus",
        aliases: &[],
        name: "Opus",
        codec: "Opus audio in an Ogg container",
        family: Family::Audio,
        lossy: true,
        blurb: "Opus delivers the best quality per bit of any mainstream codec — \
                it beats MP3, Vorbis and AAC at almost every bitrate and powers \
                WhatsApp voice notes, Discord and WebRTC. Device support, \
                though, is still patchy outside browsers and messengers.",
        uses: "voice notes, VoIP and low-bitrate streaming",
        strength: "best quality per bit of any mainstream codec",
        caveat: "patchy support on older devices, car stereos and Apple apps",
        size_note: "smallest — excellent quality even at low bitrates",
        compat: "great in browsers and messengers; patchy on older hardware",
        transparency: None,
        animation: None,
    },
    FormatInfo {
        key: "aiff",
        aliases: &[],
        name: "AIFF",
        codec: "uncompressed PCM in Apple's AIFF container",
        family: Family::Audio,
        lossy: false,
        blurb: "AIFF is Apple's classic uncompressed audio container — the Mac \
                counterpart to WAV. Same raw PCM audio, same huge files, mostly \
                seen in pro-audio sessions and older Mac workflows.",
        uses: "Mac pro-audio sessions and sample libraries",
        strength: "uncompressed PCM, like WAV; native on macOS",
        caveat: "huge files; rarer than WAV outside Apple software",
        size_note: "very large — roughly 10 MB per minute, like WAV",
        compat: "native on macOS; most editors elsewhere open it too",
        transparency: None,
        animation: None,
    },
    // ---- image ----
    FormatInfo {
        key: "png",
        aliases: &[],
        name: "PNG",
        codec: "lossless DEFLATE-compressed bitmap",
        family: Family::Image,
        lossy: false,
        blurb: "PNG is the standard lossless web image: screenshots, UI \
                graphics, logos and anything with sharp edges or transparency \
                come out pixel-perfect.",
        uses: "screenshots, graphics, logos and anything needing transparency",
        strength: "lossless, with a full alpha channel",
        caveat: "large for photographs — photos compress far better as JPEG or WebP",
        size_note: "efficient for flat graphics, large for photos",
        compat: "universal",
        transparency: Some("yes — full alpha channel"),
        animation: Some("no"),
    },
    FormatInfo {
        key: "jpeg",
        aliases: &["jpg"],
        name: "JPEG",
        codec: "lossy DCT-compressed JPEG",
        family: Family::Image,
        lossy: true,
        blurb: "JPEG is the default format for photographs — lossy compression \
                tuned for natural images keeps files small, and support is \
                universal. It has no alpha channel, so transparency is always \
                flattened.",
        uses: "photos and everyday web images",
        strength: "small files for photos; opens absolutely everywhere",
        caveat: "no transparency; visible artifacts at low quality settings",
        size_note: "small — the quality knob trades size against artifacts",
        compat: "universal",
        transparency: Some("no — transparent areas are flattened"),
        animation: Some("no"),
    },
    FormatInfo {
        key: "webp",
        aliases: &[],
        name: "WebP",
        codec: "WebP — written lossy here, with a quality knob",
        family: Family::Image,
        lossy: true,
        blurb: "WebP is the modern web format: at similar visual quality it \
                produces noticeably smaller files than both JPEG and PNG, and \
                it supports alpha transparency.",
        uses: "modern web images — photos and graphics alike",
        strength: "smaller than JPEG/PNG at similar quality, with transparency",
        caveat: "a few older apps and viewers still don't open it",
        size_note: "smallest of the web formats at comparable quality",
        compat: "all modern browsers; some older desktop software lags",
        transparency: Some("yes — alpha supported"),
        animation: Some("possible in WebP, but this converter writes still images"),
    },
    FormatInfo {
        key: "gif",
        aliases: &[],
        name: "GIF",
        codec: "LZW-compressed, 256-color palette bitmap",
        family: Family::Image,
        lossy: false,
        blurb: "GIF is the veteran web format: a 256-color palette, universal \
                support and — famously — animation. For still images its color \
                limit shows, which is why single frames usually travel better \
                as PNG, JPEG or WebP.",
        uses: "simple animations and legacy graphics",
        strength: "universally supported; can hold animation",
        caveat: "only 256 colors per frame; animation is not carried over to still formats",
        size_note: "small for flat art, poor for photos",
        compat: "universal",
        transparency: Some("1-bit — a pixel is either fully opaque or fully transparent"),
        animation: Some("yes — but never preserved by a still-image conversion"),
    },
    FormatInfo {
        key: "bmp",
        aliases: &[],
        name: "BMP",
        codec: "uncompressed Windows bitmap",
        family: Family::Image,
        lossy: false,
        blurb: "BMP is the old Windows bitmap: raw uncompressed pixels with \
                essentially no size optimization. It opens everywhere on \
                Windows but wastes space — which is usually why people convert \
                it.",
        uses: "legacy Windows software and raw exports",
        strength: "dead simple and lossless",
        caveat: "uncompressed — enormous files for what they show",
        size_note: "very large — no compression at all",
        compat: "universal on Windows; most other platforms too",
        transparency: Some("effectively none in common BMP files"),
        animation: Some("no"),
    },
    FormatInfo {
        key: "tiff",
        aliases: &[],
        name: "TIFF",
        codec: "TIFF container, usually uncompressed or LZW",
        family: Family::Image,
        lossy: false,
        blurb: "TIFF is the print-and-scan workhorse: a flexible, usually \
                lossless container beloved by scanners, print shops and photo \
                archives. Files are often huge and can even hold multiple \
                pages, which makes them awkward to share.",
        uses: "scanning, print production and photo archives",
        strength: "high-fidelity master files",
        caveat: "often huge; can hold multiple pages, and this converter is built for single images",
        size_note: "large to very large",
        compat: "great in imaging software; browsers generally won't display it",
        transparency: Some("possible, but uncommon"),
        animation: Some("no — multi-page instead"),
    },
];

/// Look up a format by canonical key or alias ("jpg" resolves to "jpeg").
pub fn lookup(key: &str) -> Option<&'static FormatInfo> {
    FORMATS
        .iter()
        .find(|f| f.key == key || f.aliases.contains(&key))
}

/// Display name for a slug segment: alias spellings keep their own casing
/// ("jpg" → "JPG") so a page's h1 matches the query the visitor typed, while
/// canonical keys use the format's display name ("jpeg" → "JPEG").
pub fn display_for_segment(segment: &str) -> String {
    match lookup(segment) {
        Some(info) if info.key == segment => info.name.to_string(),
        _ => segment.to_ascii_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_every_canonical_key() {
        for key in [
            "mp3", "wav", "ogg", "flac", "m4a", "aac", "opus", "aiff", // audio
            "png", "jpeg", "webp", "gif", "bmp", "tiff", // image
        ] {
            let info = lookup(key).unwrap_or_else(|| panic!("missing format {key}"));
            assert_eq!(info.key, key);
        }
        assert!(lookup("mid").is_none());
        assert!(lookup("").is_none());
    }

    #[test]
    fn jpg_alias_resolves_to_jpeg() {
        let info = lookup("jpg").expect("jpg alias");
        assert_eq!(info.key, "jpeg");
        assert_eq!(info.name, "JPEG");
        // Alias display keeps the typed spelling; canonical keeps the name.
        assert_eq!(display_for_segment("jpg"), "JPG");
        assert_eq!(display_for_segment("jpeg"), "JPEG");
        assert_eq!(display_for_segment("webp"), "WebP");
        assert_eq!(display_for_segment("opus"), "Opus");
    }

    #[test]
    fn families_and_lossiness_match_the_parent_tools() {
        // Lossy audio targets per blocks/audio-convert (bitrate applies):
        for k in ["mp3", "ogg", "m4a", "aac", "opus"] {
            let f = lookup(k).unwrap();
            assert_eq!(f.family, Family::Audio, "{k}");
            assert!(f.lossy, "{k} is lossy");
        }
        for k in ["wav", "flac", "aiff"] {
            let f = lookup(k).unwrap();
            assert_eq!(f.family, Family::Audio, "{k}");
            assert!(!f.lossy, "{k} is lossless");
        }
        // Image: PNG lossless (quality ignored); JPEG/WebP take the quality knob.
        assert!(!lookup("png").unwrap().lossy);
        assert!(lookup("jpeg").unwrap().lossy);
        assert!(lookup("webp").unwrap().lossy);
        for k in ["png", "jpeg", "webp", "gif", "bmp", "tiff"] {
            assert_eq!(lookup(k).unwrap().family, Family::Image, "{k}");
        }
    }

    #[test]
    fn image_formats_carry_transparency_and_animation_cells() {
        for f in FORMATS {
            match f.family {
                Family::Image => {
                    assert!(f.transparency.is_some(), "{} transparency cell", f.key);
                    assert!(f.animation.is_some(), "{} animation cell", f.key);
                }
                Family::Audio => {
                    assert!(f.transparency.is_none(), "{}", f.key);
                    assert!(f.animation.is_none(), "{}", f.key);
                }
            }
        }
    }

    #[test]
    fn every_entry_has_nonempty_copy_fragments() {
        for f in FORMATS {
            for (field, value) in [
                ("codec", f.codec),
                ("blurb", f.blurb),
                ("uses", f.uses),
                ("strength", f.strength),
                ("caveat", f.caveat),
                ("size_note", f.size_note),
                ("compat", f.compat),
            ] {
                assert!(!value.trim().is_empty(), "{} {field} empty", f.key);
            }
        }
    }
}
