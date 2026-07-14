//! Programmatic "X to Y" converter landing pages — one page per (source,
//! target) format pair of the two real converter tools, at
//! `pkg/tools/<parent>/<src>-to-<tgt>/`. These answer the queries people
//! actually type ("wav to mp3", "png to webp") and deep-link the parent
//! converter with the target preselected.
//!
//! Every section of a pair page is assembled from the per-format fact table in
//! `formats.rs` plus direction-specific copy (lossy→lossless, lossless→lossy,
//! …), so no two pages share their prose — the guard against thin/doorway
//! content. A test enforces ≥250 words of visible prose on every page.

use crate::formats::{self, Family, FormatInfo};
use crate::meta::ToolMeta;
use crate::site::SiteConfig;
use crate::template::breadcrumbs_json_ld;
use maud::{html, Markup, PreEscaped, DOCTYPE};

/// Source formats the audio converter accepts (anything ffmpeg decodes; these
/// are the ones worth a landing page) and the targets it writes.
pub const AUDIO_SOURCES: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac", "opus", "aiff"];
pub const AUDIO_TARGETS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a"];

/// Image sources (per the parent tool's input FAQ) and targets. Sources use
/// the everyday "jpg" spelling; the target keeps the tool's canonical
/// `?format=jpeg` value. `jpg` → `jpeg` counts as identity (same format).
pub const IMAGE_SOURCES: &[&str] = &["png", "jpg", "webp", "gif", "bmp", "tiff"];
pub const IMAGE_TARGETS: &[&str] = &["jpeg", "png", "webp"];

/// One source→target conversion page under a parent converter tool.
pub struct PairSpec {
    /// Parent tool slug ("audio-convert" | "image-convert").
    pub parent: &'static str,
    /// Source slug segment (may be an alias spelling, e.g. "jpg").
    pub src: &'static str,
    /// Target slug segment — always the parent tool's `?format=` value.
    pub tgt: &'static str,
}

/// How quality moves across the conversion — drives the direction-specific
/// intro, caveat and FAQ copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    LosslessToLossy,
    LossyToLossless,
    LossyToLossy,
    LosslessToLossless,
}

impl PairSpec {
    pub fn src_info(&self) -> &'static FormatInfo {
        formats::lookup(self.src).unwrap_or_else(|| panic!("unknown format {}", self.src))
    }

    pub fn tgt_info(&self) -> &'static FormatInfo {
        formats::lookup(self.tgt).unwrap_or_else(|| panic!("unknown format {}", self.tgt))
    }

    /// Display names as typed in the slug ("jpg" → "JPG", "jpeg" → "JPEG").
    pub fn src_name(&self) -> String {
        formats::display_for_segment(self.src)
    }

    pub fn tgt_name(&self) -> String {
        formats::display_for_segment(self.tgt)
    }

    /// Directory name under the parent: "wav-to-mp3".
    pub fn slug(&self) -> String {
        format!("{}-to-{}", self.src, self.tgt)
    }

    /// Sitemap/`_pairs.json` path: "audio-convert/wav-to-mp3".
    pub fn path(&self) -> String {
        format!("{}/{}", self.parent, self.slug())
    }

    pub fn h1(&self) -> String {
        format!("Convert {} to {}", self.src_name(), self.tgt_name())
    }

    /// `cfg`-branded per the site's `brand_name` — independent of
    /// `title_suffix` (see `SiteConfig::brand_title` doc comment).
    pub fn title(&self, cfg: &SiteConfig) -> String {
        cfg.brand_title(&format!(
            "Convert {} to {} Online — Free & Private",
            self.src_name(),
            self.tgt_name()
        ))
    }

    pub fn description(&self) -> String {
        let base = format!(
            "Convert {} to {} right in your browser — free, private, nothing is uploaded.",
            self.src_name(),
            self.tgt_name()
        );
        let hook = match self.src_info().family {
            Family::Audio => match self.direction() {
                Direction::LosslessToLossy => "Pick a bitrate (32–320 kbps) and shrink the file dramatically.",
                Direction::LossyToLossless => "Get a lossless file any editor opens — no further quality loss.",
                Direction::LossyToLossy => "One-click re-encode for compatibility.",
                Direction::LosslessToLossless => "A perfect, bit-for-bit lossless copy.",
            },
            Family::Image => match self.tgt_info().key {
                "jpeg" => "Small files for photos — set quality 1–100.",
                "png" => "Get a lossless copy of every pixel.",
                _ => "Smaller files at the same quality, transparency included.",
            },
        };
        format!("{base} {hook}")
    }

    /// OG-card tagline (second line on the 1200×630 card).
    pub fn og_tagline(&self) -> String {
        format!(
            "{} → {} in your browser — free & private, nothing is uploaded.",
            self.src_name(),
            self.tgt_name()
        )
    }

    /// Deep link opening the parent tool with the target preselected.
    pub fn cta_url(&self) -> String {
        match self.src_info().family {
            Family::Audio => format!("/tools/{}/?format={}", self.parent, self.tgt),
            Family::Image if self.tgt_info().lossy => {
                format!("/tools/{}/?format={}&quality=85", self.parent, self.tgt)
            }
            // PNG is lossless — the quality param is ignored, so don't teach it.
            Family::Image => format!("/tools/{}/?format={}", self.parent, self.tgt),
        }
    }

    fn direction(&self) -> Direction {
        match (self.src_info().lossy, self.tgt_info().lossy) {
            (false, true) => Direction::LosslessToLossy,
            (true, false) => Direction::LossyToLossless,
            (true, true) => Direction::LossyToLossy,
            (false, false) => Direction::LosslessToLossless,
        }
    }
}

/// Enumerate every pair page: sources × targets per family, identity skipped
/// (compared on canonical format keys, so jpg→jpeg is identity).
pub fn all_pairs() -> Vec<PairSpec> {
    let mut out = Vec::new();
    let families: [(&str, &[&str], &[&str]); 2] = [
        ("audio-convert", AUDIO_SOURCES, AUDIO_TARGETS),
        ("image-convert", IMAGE_SOURCES, IMAGE_TARGETS),
    ];
    for (parent, sources, targets) in families {
        for src in sources {
            for tgt in targets {
                let same = formats::lookup(src).map(|f| f.key)
                    == formats::lookup(tgt).map(|f| f.key);
                if !same {
                    out.push(PairSpec { parent, src, tgt });
                }
            }
        }
    }
    out
}

/// A "Popular conversions" link on the parent tool's page.
pub struct PairLink {
    pub label: String,
    pub href: String,
}

/// The parent page's crawlable link list to all its pair pages.
pub fn pair_links_for_parent(pairs: &[PairSpec], parent_slug: &str) -> Vec<PairLink> {
    pairs
        .iter()
        .filter(|p| p.parent == parent_slug)
        .map(|p| PairLink {
            label: format!("{} to {}", p.src_name(), p.tgt_name()),
            href: format!("/tools/{}/", p.path()),
        })
        .collect()
}

/// `pkg/tools/_pairs.json` — `[{path, title}]`, consumed by
/// `scripts/gen-seo.sh` to add the pair URLs to the sitemap.
pub fn pairs_json(cfg: &SiteConfig, pairs: &[&PairSpec]) -> String {
    let entries: Vec<serde_json::Value> = pairs
        .iter()
        .map(|p| serde_json::json!({ "path": p.path(), "title": p.title(cfg) }))
        .collect();
    serde_json::to_string(&entries).expect("serialize pairs index")
}

// ---------------------------------------------------------------------------
// Page content assembly — every fragment below is parameterized by the pair's
// formats and direction, so the prose is pair-specific by construction.
// ---------------------------------------------------------------------------

/// Indefinite article for a format's display name, by pronunciation
/// ("an MP3", "an AIFF" — but "a WAV", "a FLAC").
fn article(name: &str) -> &'static str {
    match name {
        "MP3" | "M4A" | "OGG" | "AAC" | "Opus" | "AIFF" => "an",
        _ => "a",
    }
}

/// Why-this-conversion intro paragraph, direction- and pair-specific.
fn why_paragraph(pair: &PairSpec) -> String {
    let src = pair.src_name();
    let tgt = pair.tgt_name();
    let tgt_info = pair.tgt_info();
    match pair.src_info().family {
        Family::Audio => match pair.direction() {
            Direction::LosslessToLossy => format!(
                "Converting {src} to {tgt} trades a sliver of fidelity for a dramatic drop \
                 in size: {} {src} recording that hogs storage becomes {} {tgt} file you \
                 can email, message or stream. It's the classic move for sharing voice \
                 memos, publishing spoken audio and fitting a music library onto a phone.",
                article(&src),
                article(&tgt)
            ),
            Direction::LossyToLossless => format!(
                "Converting {src} to {tgt} is about compatibility rather than quality: some \
                 editors, DAWs and pipelines simply insist on {tgt}. You get a file they \
                 open natively — but the audio can never get better than the {src} you \
                 start from."
            ),
            Direction::LossyToLossy => format!(
                "Converting {src} to {tgt} is a compatibility move: both formats are lossy, \
                 so the goal isn't quality — it's producing a file that fits {}. Expect the \
                 sound to stay essentially the same, with a small second round of encoding \
                 loss.",
                tgt_info.uses
            ),
            Direction::LosslessToLossless => match (pair.src_info().key, tgt_info.key) {
                (_, "flac") => format!(
                    "Converting {src} to FLAC is a pure win for storage: the copy is \
                     bit-for-bit identical to the original, at typically 50–70% of the \
                     {src} size."
                ),
                ("flac", _) => format!(
                    "Converting FLAC to {tgt} unpacks the compressed audio back into raw \
                     PCM: the file grows several times over, but you get a format every \
                     editor and OS opens without any codec support."
                ),
                _ => format!(
                    "Converting {src} to {tgt} is essentially a container swap: the raw PCM \
                     samples are identical and the file size barely changes — you do it \
                     when a tool or plugin only accepts {tgt}."
                ),
            },
        },
        Family::Image => match tgt_info.key {
            "jpeg" => format!(
                "Converting {src} to {tgt} is almost always about size: JPEG's photo-tuned \
                 lossy compression produces far smaller files than {src} for photographic \
                 content, and every app on earth opens the result."
            ),
            "webp" => format!(
                "Converting {src} to WebP usually cuts file size at the same visual \
                 quality — WebP out-compresses {src} on most images — while keeping alpha \
                 transparency available."
            ),
            // PNG target: the story depends on where you're coming from.
            _ if pair.src_info().lossy => format!(
                "Converting {src} to PNG is about compatibility, not quality: PNG stores \
                 pixels losslessly and is accepted everywhere, so it's the format to reach \
                 for when an upload form or workflow strictly requires it. The {src}'s \
                 compression artifacts, though, are already baked into the pixels."
            ),
            _ => match pair.src_info().key {
                "gif" => "Converting GIF to PNG modernizes a still image: PNG keeps every \
                          pixel, supports proper alpha transparency and has no 256-color \
                          ceiling for future edits."
                    .to_string(),
                "bmp" => "Converting BMP to PNG keeps every pixel and throws away the \
                          wasted space: PNG's lossless compression routinely shrinks an \
                          uncompressed bitmap by a large factor."
                    .to_string(),
                _ => "Converting TIFF to PNG turns a heavyweight print-and-scan master \
                      into something browsers and chat apps actually display — losslessly, \
                      so nothing is thrown away."
                    .to_string(),
            },
        },
    }
}

/// The src-vs-tgt comparison table.
fn comparison_table(pair: &PairSpec) -> Markup {
    let (src, tgt) = (pair.src_info(), pair.tgt_info());
    let type_cell = |f: &FormatInfo| {
        if f.lossy {
            "lossy — some detail traded for size"
        } else {
            "lossless — nothing discarded"
        }
    };
    html! {
        table {
            thead {
                tr {
                    th {}
                    th { (pair.src_name()) }
                    th { (pair.tgt_name()) }
                }
            }
            tbody {
                tr { th { "Compression" } td { (type_cell(src)) } td { (type_cell(tgt)) } }
                tr { th { "Codec / container" } td { (src.codec) } td { (tgt.codec) } }
                tr { th { "Typical file size" } td { (src.size_note) } td { (tgt.size_note) } }
                tr { th { "Best for" } td { (src.uses) } td { (tgt.uses) } }
                tr { th { "Strength" } td { (src.strength) } td { (tgt.strength) } }
                tr { th { "Watch out for" } td { (src.caveat) } td { (tgt.caveat) } }
                tr { th { "Compatibility" } td { (src.compat) } td { (tgt.compat) } }
                @if let (Some(s), Some(t)) = (src.transparency, tgt.transparency) {
                    tr { th { "Transparency" } td { (s) } td { (t) } }
                }
                @if let (Some(s), Some(t)) = (src.animation, tgt.animation) {
                    tr { th { "Animation" } td { (s) } td { (t) } }
                }
            }
        }
    }
}

/// The 3-step "How the conversion works" list.
fn how_it_works(pair: &PairSpec) -> Markup {
    let src = pair.src_name();
    let tgt = pair.tgt_name();
    let audio = pair.src_info().family == Family::Audio;
    let step1 = if audio {
        format!(
            "Choose your {src} file (up to 10 MiB). The button above opens the converter \
             with {tgt} already selected as the target format."
        )
    } else {
        format!(
            "Choose your {src} image. The button above opens the converter with {tgt} \
             already selected as the target format."
        )
    };
    let step2 = match (audio, pair.tgt_info().lossy) {
        (true, true) => "Pick a bitrate between 32 and 320 kbps — the default 192 kbps is \
                         transparent for most material, and values outside the range are \
                         clamped."
            .to_string(),
        (true, false) => format!(
            "There is no bitrate to choose: {tgt} is lossless, so the bitrate field is \
             simply ignored."
        ),
        (false, true) => "Set the quality from 1 to 100 (default 85) — higher keeps more \
                          detail, lower shrinks the file."
            .to_string(),
        (false, false) => "There is no quality to tune: PNG is lossless, so the quality \
                           setting is ignored."
            .to_string(),
    };
    let step3 = if audio {
        format!(
            "Run the conversion and download the result — the output keeps your filename \
             with a .{} extension. Everything happens locally: the page runs ffmpeg \
             compiled to WebAssembly, so your audio is never uploaded to a server.",
            pair.tgt
        )
    } else {
        format!(
            "Run the conversion and download the {tgt} image. Everything happens locally — \
             ffmpeg compiled to WebAssembly runs in your browser tab, your image is never \
             uploaded, and the page keeps working offline once it has loaded."
        )
    };
    html! {
        ol {
            li { (step1) }
            li { (step2) }
            li { (step3) }
        }
    }
}

/// Direction- and pair-specific "What to expect" paragraphs.
fn caveats(pair: &PairSpec) -> Vec<Markup> {
    let src = pair.src_name();
    let tgt = pair.tgt_name();
    let mut out = Vec::new();
    match pair.src_info().family {
        Family::Audio => {
            out.push(match pair.direction() {
                Direction::LossyToLossless => html! {
                    p {
                        strong { "No quality is restored. " }
                        (format!(
                            "{tgt} preserves exactly what's in your {src} file — detail the \
                             {src} encoder already discarded is gone for good. Expect a much \
                             larger file with identical sound; convert because a tool needs \
                             {tgt}, not to upgrade the audio."
                        ))
                    }
                },
                Direction::LosslessToLossy => html! {
                    p {
                        strong { "This step is lossy. " }
                        (format!(
                            "The bitrate decides how much detail the {tgt} keeps: 192 kbps \
                             (the default) is transparent for most music, 128 kbps suits \
                             voice recordings, and 256–320 kbps is the choice for archiving. \
                             Your {src} original keeps every sample, so hold on to it."
                        ))
                    }
                },
                Direction::LossyToLossy => html! {
                    p {
                        strong { "Generation loss stacks. " }
                        (format!(
                            "Re-encoding lossy {src} audio with another lossy codec adds a \
                             second round of loss. Keep the {tgt} bitrate at or above the \
                             original's and avoid repeated round-trips between formats."
                        ))
                    }
                },
                Direction::LosslessToLossless => html! {
                    p {
                        strong { "Nothing is lost either way. " }
                        (format!(
                            "{src} and {tgt} both store audio losslessly, so the samples \
                             come through bit for bit — only packaging and file size change. \
                             The bitrate setting doesn't apply to a lossless target."
                        ))
                    }
                },
            });
            // The tool always passes -vn: attached-picture streams would break
            // audio-only muxers, so cover art never survives (parent docs).
            out.push(html! {
                p {
                    (format!(
                        "Embedded album art is dropped along the way: cover images ride \
                         along as a video stream, which audio-only outputs like {tgt} can't \
                         carry."
                    ))
                }
            });
        }
        Family::Image => {
            out.push(match pair.direction() {
                Direction::LosslessToLossy => html! {
                    p {
                        strong { "This step is lossy. " }
                        (format!(
                            "{tgt} discards some detail to hit its file sizes — at the \
                             default quality of 85 that's hard to see on photos, but \
                             sharp-edged graphics and text show artifacts sooner. Raise the \
                             quality toward 100 for critical images, and keep the {src} \
                             original in case you need to re-export."
                        ))
                    }
                },
                Direction::LossyToLossless => html! {
                    p {
                        strong { "No detail is restored. " }
                        (format!(
                            "PNG stores pixels losslessly, but your {src}'s compression \
                             artifacts are already baked into those pixels — expect a \
                             noticeably larger file with exactly the same visual quality."
                        ))
                    }
                },
                Direction::LossyToLossy => html! {
                    p {
                        strong { "Generation loss stacks. " }
                        (format!(
                            "Both {src} and {tgt} are lossy, so each re-encode discards a \
                             little more. One conversion at quality 85 is rarely visible; \
                             repeated round-trips are."
                        ))
                    }
                },
                Direction::LosslessToLossless => html! {
                    p {
                        strong { "Nothing is lost. " }
                        (format!(
                            "{src} and {tgt} both store the image losslessly, so every \
                             pixel comes through exactly — and the quality setting doesn't \
                             even apply to a PNG target."
                        ))
                    }
                },
            });
            let src_alpha = matches!(pair.src_info().key, "png" | "webp" | "gif");
            match pair.tgt_info().key {
                "jpeg" if src_alpha => out.push(html! {
                    p {
                        strong { "Transparency is flattened. " }
                        (format!(
                            "JPEG has no alpha channel, so transparent regions of your \
                             {src} are merged onto a solid background. If you need to keep \
                             transparency while shrinking the file, convert to WebP instead."
                        ))
                    }
                }),
                "webp" | "png" if src_alpha => out.push(html! {
                    p {
                        (format!(
                            "Transparency survives: {tgt} supports alpha, so the \
                             transparent areas of your {src} stay transparent."
                        ))
                    }
                }),
                _ => {}
            }
            if pair.src_info().key == "gif" {
                out.push(html! {
                    p {
                        strong { "Animation doesn't carry over. " }
                        (format!(
                            "{tgt} output from this converter is a single still image — the \
                             tool is built for stills, so an animated GIF won't come out \
                             animated. Keep the GIF (or convert it to a video format) when \
                             you need motion."
                        ))
                    }
                });
            }
            if pair.src_info().key == "tiff" {
                out.push(html! {
                    p {
                        strong { "Multi-page TIFFs: " }
                        "TIFF can hold several pages in one file, and this converter is \
                         designed for single-image files — split multi-page documents \
                         first."
                    }
                });
            }
        }
    }
    out
}

/// 2–3 pair-specific FAQs as (question, answer) pairs. Rendered as
/// `<h2>FAQ</h2>` + `<details>` blocks, which `faq::extract_faqs` picks up so
/// the page emits matching FAQPage JSON-LD automatically.
fn faqs(pair: &PairSpec) -> Vec<(String, String)> {
    let src = pair.src_name();
    let tgt = pair.tgt_name();
    let mut out = Vec::new();
    match pair.src_info().family {
        Family::Audio => {
            out.push(match pair.direction() {
                Direction::LosslessToLossy => (
                    format!("How much quality do I lose converting {src} to {tgt}?"),
                    format!(
                        "At the default 192 kbps, {tgt} is transparent for most listeners \
                         and most material — you'd struggle to tell it from the {src} \
                         original. Push the bitrate to 256–320 kbps for archiving, or drop \
                         to 128 kbps for voice recordings where size matters most."
                    ),
                ),
                Direction::LossyToLossless => (
                    format!("Does converting {src} to {tgt} improve the audio quality?"),
                    format!(
                        "No. {tgt} preserves exactly what's in the source — detail the \
                         {src} encoder already discarded is gone for good. Convert because \
                         a tool needs {tgt}, not to upgrade the sound."
                    ),
                ),
                Direction::LossyToLossy => (
                    format!("Will converting {src} to {tgt} make my audio sound worse?"),
                    format!(
                        "Marginally, in principle: both formats are lossy, so the re-encode \
                         adds a second generation of loss. At 192 kbps or higher it's \
                         rarely audible, but keep the original {src} and avoid converting \
                         back and forth."
                    ),
                ),
                Direction::LosslessToLossless => (
                    format!("Is converting {src} to {tgt} really lossless?"),
                    "Yes — both formats store the audio losslessly, so the conversion is \
                     bit-transparent. What changes is packaging and file size, not sound."
                        .to_string(),
                ),
            });
            out.push(if pair.tgt_info().lossy {
                (
                    format!("What bitrate should I pick for the {tgt} file?"),
                    format!(
                        "The converter accepts 32–320 kbps and defaults to 192 kbps, which \
                         is a good balance for music. Use 128 kbps for voice where size \
                         matters and 256–320 kbps for archiving{}. Values outside the range \
                         are clamped.",
                        if pair.src_info().lossy {
                            format!(
                                "; when re-encoding from {src}, match or exceed the \
                                 source's bitrate to limit further loss"
                            )
                        } else {
                            String::new()
                        }
                    ),
                )
            } else {
                match (pair.src_info().key, pair.tgt_info().key) {
                    (s, "flac") if s != "flac" && !pair.src_info().lossy => (
                        "How much smaller will the FLAC file be?".to_string(),
                        format!(
                            "Typically 50–70% of the {src} size — FLAC's compression is \
                             lossless, so the saving comes free. Exact ratios depend on the \
                             material: quiet, simple audio shrinks further than dense, loud \
                             mixes."
                        ),
                    ),
                    ("flac", _) => (
                        format!("Why is the {tgt} so much bigger than the FLAC?"),
                        format!(
                            "{tgt} is raw, uncompressed PCM — roughly 10 MB per minute of \
                             16-bit stereo — while FLAC stores the same samples compressed. \
                             Unpacking is the price of universal, codec-free compatibility."
                        ),
                    ),
                    ("aiff", "wav") => (
                        "What actually changes between AIFF and WAV?".to_string(),
                        "Only the container. Both hold raw PCM samples, so the audio bytes \
                         and the file size stay essentially the same — the point is \
                         compatibility with tools that only read WAV."
                            .to_string(),
                    ),
                    _ => (
                        format!("Why is the {tgt} file so much larger than my {src}?"),
                        format!(
                            "{src} stores heavily compressed audio; {tgt} {} — so the same \
                             sound takes several times the space. That's normal, and the \
                             extra bytes don't add quality.",
                            if pair.tgt_info().key == "wav" {
                                "expands it to raw PCM (about 10 MB per minute of stereo)"
                            } else {
                                "stores a losslessly-compressed copy of the decoded audio"
                            }
                        ),
                    ),
                }
            });
            out.push((
                format!("Is my {src} file uploaded when converting to {tgt}?"),
                "No. The page downloads an ffmpeg WebAssembly build once, then converts \
                 your file locally in the browser tab — the audio never leaves your \
                 device. Input files up to 10 MiB are supported."
                    .to_string(),
            ));
        }
        Family::Image => {
            out.push(match pair.direction() {
                Direction::LosslessToLossy => (
                    format!("How much quality do I lose converting {src} to {tgt}?"),
                    format!(
                        "At the default quality of 85 the difference is hard to spot on \
                         photos. Graphics with sharp edges and text show lossy artifacts \
                         sooner — try quality 90+ there, or stick with a lossless format. \
                         Your {src} original is untouched either way."
                    ),
                ),
                Direction::LossyToLossless => (
                    format!("Will converting {src} to PNG make it sharper?"),
                    format!(
                        "No. PNG stores the pixels losslessly, but the {src} compression \
                         artifacts are already baked into those pixels — you get a larger \
                         file with exactly the same visual quality. Convert when a workflow \
                         strictly requires PNG."
                    ),
                ),
                Direction::LossyToLossy => (
                    format!("Will converting {src} to {tgt} lose quality?"),
                    format!(
                        "A little, in principle: both {src} and {tgt} are lossy, so the \
                         re-encode discards a bit more. At the default quality of 85 the \
                         difference is rarely visible — but keep the original if you expect \
                         to convert again."
                    ),
                ),
                Direction::LosslessToLossless => (
                    format!("Is converting {src} to PNG lossless?"),
                    format!(
                        "Yes — PNG keeps every pixel of the decoded {src} exactly. {}",
                        match pair.src_info().key {
                            "gif" =>
                                "The GIF's 256-color palette still limits the source \
                                 detail: PNG can hold more colors, but it can't invent ones \
                                 the GIF never had.",
                            "bmp" => "You keep the image and simply drop the uncompressed bulk.",
                            _ => "For a multi-page TIFF, that applies to the single image \
                                  being converted.",
                        }
                    ),
                ),
            });
            let src_alpha = matches!(pair.src_info().key, "png" | "webp" | "gif");
            out.push(if pair.src_info().key == "gif" {
                (
                    format!("Does an animated GIF stay animated as {tgt}?"),
                    format!(
                        "No. {tgt} output from this converter is a single still image — \
                         the tool is built for stills, so animation is never preserved. \
                         Keep the GIF, or use a video tool, when you need motion."
                    ),
                )
            } else if pair.tgt_info().key == "jpeg" && src_alpha {
                (
                    format!("What happens to transparency when I convert {src} to JPEG?"),
                    format!(
                        "It's lost — JPEG has no alpha channel, so transparent regions of \
                         the {src} get flattened onto a solid background. If you need to \
                         keep transparency while shrinking the file, convert to WebP \
                         instead."
                    ),
                )
            } else if matches!(pair.src_info().key, "bmp" | "tiff") {
                (
                    format!("How much smaller will the {tgt} be than my {src}?"),
                    if pair.tgt_info().lossy {
                        format!(
                            "Usually dramatically smaller: {src} files carry little to no \
                             compression, while {tgt} at the default quality of 85 \
                             compresses photographic content to a small fraction of that."
                        )
                    } else {
                        format!(
                            "Usually much smaller: PNG compresses losslessly, so you keep \
                             every pixel while shedding most of the {src}'s bulk — with no \
                             quality change at all."
                        )
                    },
                )
            } else if pair.tgt_info().key == "png" {
                (
                    format!("Why is the PNG bigger than my {src}?"),
                    format!(
                        "{src} stores the image as heavily compressed lossy data; PNG \
                         re-stores the decoded pixels losslessly, which simply takes more \
                         bytes. Same look, bigger file — worth it only when PNG is \
                         required."
                    ),
                )
            } else {
                (
                    "How does the quality setting work for WebP?".to_string(),
                    "Quality runs from 1 to 100 (default 85) and is mapped onto ffmpeg's \
                     encoder scale. Higher values keep more detail and grow the file; for \
                     most images 80–90 is the sweet spot. It only applies to lossy targets \
                     — PNG ignores it."
                        .to_string(),
                )
            });
            out.push((
                format!("Is my {src} image uploaded when converting to {tgt}?"),
                "No. The conversion runs entirely in your browser with ffmpeg compiled to \
                 WebAssembly — your image never leaves your device, and the page keeps \
                 working offline once it has loaded."
                    .to_string(),
            ));
        }
    }
    out
}

/// The page's prose content (everything inside `<section class="tool-content">`),
/// as an HTML string — also what the ≥250-word guard measures and what
/// `faq::extract_faqs` reads for the FAQPage JSON-LD.
pub fn content_html(pair: &PairSpec) -> String {
    let markup = html! {
        p { (pair.src_info().blurb) }
        p { (pair.tgt_info().blurb) }
        p { (why_paragraph(pair)) }
        h2 { (format!("{} vs {}", pair.src_name(), pair.tgt_name())) }
        (comparison_table(pair))
        h2 { "How the conversion works" }
        (how_it_works(pair))
        h2 { "What to expect" }
        @for para in caveats(pair) { (para) }
        h2 { "FAQ" }
        @for (q, a) in faqs(pair) {
            details {
                summary { (q) }
                p { (a) }
            }
        }
    };
    markup.into_string()
}

/// Sibling-links section: the other targets for the same source, the reverse
/// direction when it exists, and the parent converter.
fn siblings_section(parent: &ToolMeta, pair: &PairSpec, all: &[PairSpec]) -> Markup {
    let others: Vec<&PairSpec> = all
        .iter()
        .filter(|p| p.parent == pair.parent && p.src == pair.src && p.tgt != pair.tgt)
        .collect();
    let reverse = all
        .iter()
        .find(|p| p.parent == pair.parent && p.src_info().key == pair.tgt_info().key
            && p.tgt_info().key == pair.src_info().key);
    html! {
        section class="pair-siblings" {
            h2 { (format!("More {} conversions", pair.src_name())) }
            ul class="pair-siblings-list" {
                @for o in &others {
                    li { a href=(format!("/tools/{}/", o.path())) { (format!("{} to {}", o.src_name(), o.tgt_name())) } }
                }
                @if let Some(r) = reverse {
                    li { a href=(format!("/tools/{}/", r.path())) { (format!("{} to {} (reverse)", r.src_name(), r.tgt_name())) } }
                }
                li { a href=(format!("/tools/{}/", pair.parent)) { (format!("All formats — {}", parent.h1)) } }
            }
        }
    }
}

/// Inline styles for the pair-page hero, CTA and sibling chips. Everything
/// else (prose, tables, FAQ details) reuses `.tool-content` from the parent's
/// `tool.css`, which the page links relatively (`../tool.css`).
const PAIR_CSS: &str = r#"
.pair-hero { max-width: 720px; margin: 8px auto 0; text-align: center; padding: 12px 0 8px; }
.pair-hero h1 { font-size: clamp(24px, 5vw, 34px); font-weight: 800; margin: 10px 0 6px; }
.pair-breadcrumb { font-size: .85rem; color: var(--tool-muted, #6b7280); margin: 0; }
.pair-breadcrumb a { color: inherit; text-decoration: none; }
.pair-breadcrumb a:hover { text-decoration: underline; }
.pair-sub { color: var(--tool-muted, #6b7280); max-width: 560px; margin: 0 auto; line-height: 1.55; }
.pair-cta { display: inline-block; margin: 18px auto 4px; padding: 13px 26px; background: var(--tool-accent, #4f46e5); color: #fff; font-weight: 650; border-radius: 10px; text-decoration: none; box-shadow: 0 4px 14px rgba(79,70,229,.35); }
.pair-cta:hover, .pair-cta:focus-visible { background: #4338ca; }
.pair-cta-note { font-size: .85rem; color: var(--tool-muted, #6b7280); margin: 6px 0 0; }
.pair-siblings { max-width: 720px; margin: 40px auto 64px; }
.pair-siblings h2 { font-size: 1.3rem; margin: 0 0 12px; color: var(--tool-ink, #0f172a); border-bottom: 1px solid #e2e8f0; padding-bottom: 8px; }
.pair-siblings-list { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 8px; }
.pair-siblings-list a { display: inline-block; padding: 7px 14px; border: 1px solid #e2e8f0; border-radius: 999px; background: #fff; color: var(--tool-accent, #4f46e5); font-size: .9rem; text-decoration: none; }
.pair-siblings-list a:hover, .pair-siblings-list a:focus-visible { border-color: var(--tool-accent, #4f46e5); text-decoration: underline; }
"#;

/// Render the full HTML document for one pair page. Chrome assets are linked
/// from the PARENT tool's directory (`../tool.css`, `../header.js`, …) — the
/// parent page always ships them, so pair pages add no per-page asset copies.
pub fn render_pair_page(cfg: &SiteConfig, parent: &ToolMeta, pair: &PairSpec, all: &[PairSpec]) -> String {
    let canonical = cfg.abs(&format!("/tools/{}/", pair.path()));
    let title = pair.title(cfg);
    let description = pair.description();
    let h1 = pair.h1();
    let og_image = cfg.abs(&format!("/tools/{}/og.png", pair.path()));
    let pair_label = format!("{} to {}", pair.src_name(), pair.tgt_name());
    // BreadcrumbList JSON-LD only makes sense tied to a canonical identity —
    // omitted entirely for the unbranded/generic render.
    let breadcrumbs_ld = (!cfg.base_url.is_empty()).then(|| {
        let parent_url = cfg.abs(&format!("/tools/{}/", parent.slug)).unwrap();
        breadcrumbs_json_ld(&[
            ("Home", &cfg.abs("/").unwrap()),
            ("Tools", &cfg.abs("/tools/").unwrap()),
            (&parent.h1, &parent_url),
            (&pair_label, canonical.as_deref().unwrap()),
        ])
    });
    let content = content_html(pair);
    // FAQPage JSON-LD mirrors the visible FAQ <details> blocks — same
    // extraction path as the tool pages (one source of truth, no drift).
    let faq_entries = crate::faq::extract_faqs(&content);
    let faq_ld = (!faq_entries.is_empty()).then(|| {
        serde_json::json!({
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": faq_entries.iter().map(|f| serde_json::json!({
                "@type": "Question",
                "name": f.question,
                "acceptedAnswer": { "@type": "Answer", "text": f.answer_html },
            })).collect::<Vec<_>>(),
        })
        .to_string()
        .replace("</", "<\\/")
    });

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                meta name="description" content=(description);
                @if let Some(c) = &canonical {
                    link rel="canonical" href=(c);
                }
                link rel="alternate" type="text/markdown" href="index.md";
                meta property="og:type" content="website";
                meta property="og:title" content=(title);
                meta property="og:description" content=(description);
                @if let Some(c) = &canonical {
                    meta property="og:url" content=(c);
                }
                @if let Some(img) = &og_image {
                    meta property="og:image" content=(img);
                    meta property="og:image:width" content="1200";
                    meta property="og:image:height" content="630";
                    meta property="og:image:alt" content=(title);
                }
                meta name="twitter:card" content="summary_large_image";
                meta name="twitter:title" content=(title);
                meta name="twitter:description" content=(description);
                @if let Some(img) = &og_image {
                    meta name="twitter:image" content=(img);
                }
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                link rel="stylesheet" href="../tool.css";
                link rel="stylesheet" href="../header.css";
                (PreEscaped(cfg.head_extras.clone()))
                style { (PreEscaped(PAIR_CSS)) }
                @if let Some(bld) = &breadcrumbs_ld {
                    script type="application/ld+json" { (PreEscaped(bld)) }
                }
                @if let Some(faq_ld) = &faq_ld {
                    script type="application/ld+json" { (PreEscaped(faq_ld)) }
                }
                script type="module" src="../header.js" {}
            }
            body {
                (PreEscaped(cfg.header_html_fragment().to_string()))
                main class="tool-main" {
                    section class="pair-hero" {
                        p class="pair-breadcrumb" {
                            a href="/" { "Home" }
                            " › "
                            a href="/tools/" { "Tools" }
                            " › "
                            a href=(format!("/tools/{}/", parent.slug)) { (parent.h1) }
                            " › "
                            span { (pair_label) }
                        }
                        h1 { (h1) }
                        p class="pair-sub" { (description) }
                        a class="pair-cta" href=(pair.cta_url()) {
                            (format!("Convert {} to {} →", pair.src_name(), pair.tgt_name()))
                        }
                        p class="pair-cta-note" {
                            "Free · Private — runs in your browser, nothing is uploaded"
                        }
                    }
                    section class="tool-content" {
                        (PreEscaped(content))
                    }
                    (siblings_section(parent, pair, all))
                }
                (PreEscaped(cfg.footer_html_fragment().to_string()))
            }
        }
    };
    markup.into_string()
}

/// Markdown twin of a pair page: what/why, the deep-link CTA, a runnable CLI
/// example and a pointer at the parent tool's full docs.
pub fn pair_markdown(cfg: &SiteConfig, parent: &ToolMeta, pair: &PairSpec) -> String {
    let cli = match pair.src_info().family {
        Family::Audio => format!(
            "gizza tool {} 'url=https://example.com/input.{}' 'format={}'",
            parent.slug, pair.src, pair.tgt
        ),
        Family::Image if pair.tgt_info().lossy => format!(
            "gizza tool {} 'url=https://example.com/input.{}' 'format={}' 'quality=85'",
            parent.slug, pair.src, pair.tgt
        ),
        Family::Image => format!(
            "gizza tool {} 'url=https://example.com/input.{}' 'format={}'",
            parent.slug, pair.src, pair.tgt
        ),
    };
    format!(
        "# {h1}\n\n{desc}\n\n{src_blurb}\n\n{tgt_blurb}\n\n{why}\n\n## Run it\n\n\
         - **Web ({tgt} preselected):** {web}\n\
         - **CLI:** `{cli}`\n\
         - **Parent tool docs:** {parent_docs}\n",
        h1 = pair.h1(),
        desc = pair.description(),
        src_blurb = pair.src_info().blurb,
        tgt_blurb = pair.tgt_info().blurb,
        why = why_paragraph(pair),
        tgt = pair.tgt_name(),
        web = cfg.url_or_rel(&pair.cta_url()),
        cli = cli,
        parent_docs = cfg.url_or_rel(&format!("/tools/{}/index.md", parent.slug)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branded() -> SiteConfig {
        SiteConfig { base_url: "https://gizza.ai".into(), brand_name: "gizza.ai".into(), ..Default::default() }
    }

    fn audio_meta() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "audio-convert"
title         = "Convert Audio Online"
description   = "d"
h1            = "Convert an Audio File"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "audio"
"#,
        )
        .unwrap()
    }

    fn image_meta() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-convert"
title         = "Convert an Image Online"
description   = "d"
h1            = "Convert an Image"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "image"
"#,
        )
        .unwrap()
    }

    fn find<'a>(pairs: &'a [PairSpec], src: &str, tgt: &str) -> &'a PairSpec {
        pairs
            .iter()
            .find(|p| p.src == src && p.tgt == tgt)
            .unwrap_or_else(|| panic!("pair {src}-to-{tgt} missing"))
    }

    /// Visible text of an HTML fragment: tags stripped, whitespace collapsed.
    fn strip_tags(html: &str) -> String {
        let mut out = String::new();
        let mut in_tag = false;
        for c in html.chars() {
            match c {
                '<' => {
                    in_tag = true;
                    out.push(' ');
                }
                '>' => in_tag = false,
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn enumerates_35_audio_and_15_image_pairs_without_identity() {
        let pairs = all_pairs();
        let audio: Vec<_> = pairs.iter().filter(|p| p.parent == "audio-convert").collect();
        let image: Vec<_> = pairs.iter().filter(|p| p.parent == "image-convert").collect();
        assert_eq!(audio.len(), 35, "8 sources × 5 targets − 5 identity");
        assert_eq!(image.len(), 15, "6 sources × 3 targets − 3 identity");
        assert_eq!(pairs.len(), 50);
        // no identity pairs — jpg→jpeg counts as identity (same format)
        assert!(!pairs.iter().any(|p| p.src == p.tgt));
        assert!(!pairs.iter().any(|p| p.src == "jpg" && p.tgt == "jpeg"));
        // slugs and titles are unique across the whole set
        let mut slugs: Vec<String> = pairs.iter().map(|p| p.path()).collect();
        slugs.sort();
        slugs.dedup();
        assert_eq!(slugs.len(), 50, "pair paths unique");
        let mut titles: Vec<String> = pairs.iter().map(|p| p.title(&branded())).collect();
        titles.sort();
        titles.dedup();
        assert_eq!(titles.len(), 50, "pair titles unique");
    }

    #[test]
    fn wav_to_mp3_page_has_seo_head_breadcrumbs_cta_and_faq_ld() {
        let pairs = all_pairs();
        let pair = find(&pairs, "wav", "mp3");
        let html = render_pair_page(&branded(), &audio_meta(), pair, &pairs);
        assert!(html.contains("<title>Convert WAV to MP3 Online — Free &amp; Private — gizza.ai</title>"));
        assert!(html.contains(
            r#"<link rel="canonical" href="https://gizza.ai/tools/audio-convert/wav-to-mp3/">"#
        ));
        assert!(html.contains("<h1>Convert WAV to MP3</h1>"));
        // og card is the per-pair image
        assert!(html.contains(
            r#"content="https://gizza.ai/tools/audio-convert/wav-to-mp3/og.png""#
        ));
        // 4-level breadcrumb trail: Home › Tools › parent h1 › pair
        assert!(html.contains(r#""@type":"BreadcrumbList""#));
        assert!(html.contains(r#""item":"https://gizza.ai/tools/audio-convert/","name":"Convert an Audio File""#));
        assert!(html.contains(r#""name":"WAV to MP3""#));
        // prominent CTA deep-links the parent with the target preselected
        assert!(html.contains(r#"<a class="pair-cta" href="/tools/audio-convert/?format=mp3">"#));
        // FAQPage JSON-LD extracted from the visible <details> FAQ
        assert!(html.contains(r#""@type":"FAQPage""#), "FAQPage JSON-LD present");
        assert!(html.contains("<h2>FAQ</h2>"), "visible FAQ section");
        // markdown twin discovery link
        assert!(html.contains(r#"<link rel="alternate" type="text/markdown" href="index.md">"#));
        // chrome assets come from the parent directory
        assert!(html.contains(r#"href="../tool.css""#));
        assert!(html.contains(r#"src="../header.js""#));
        // audio pages state the documented 10 MiB input limit
        assert!(html.contains("10 MiB"));
    }

    #[test]
    fn png_to_webp_page_deep_links_quality_and_keeps_transparency_claims() {
        let pairs = all_pairs();
        let pair = find(&pairs, "png", "webp");
        let html = render_pair_page(&branded(), &image_meta(), pair, &pairs);
        assert!(html.contains(r#"href="/tools/image-convert/?format=webp&amp;quality=85""#));
        assert!(html.contains("<h1>Convert PNG to WebP</h1>"));
        // alpha survives png→webp — the page must say so, not warn about flattening
        assert!(html.contains("Transparency survives"));
        assert!(!html.contains("Transparency is flattened"));
        // image pages must NOT inherit the audio-only 10 MiB claim
        assert!(!html.contains("10 MiB"));
        // png target CTA (lossless) never teaches the ignored quality param
        let png_tgt = find(&pairs, "jpg", "png");
        assert_eq!(png_tgt.cta_url(), "/tools/image-convert/?format=png");
    }

    #[test]
    fn png_to_jpeg_warns_transparency_but_jpg_to_png_does_not() {
        let pairs = all_pairs();
        let warn = render_pair_page(&branded(), &image_meta(), find(&pairs, "png", "jpeg"), &pairs);
        assert!(warn.contains("Transparency is flattened"));
        let no_warn = render_pair_page(&branded(), &image_meta(), find(&pairs, "jpg", "png"), &pairs);
        assert!(!no_warn.contains("Transparency is flattened"));
        // gif sources warn about animation instead
        let gif = render_pair_page(&branded(), &image_meta(), find(&pairs, "gif", "png"), &pairs);
        assert!(gif.contains("Animation doesn't carry over"));
    }

    #[test]
    fn every_pair_page_has_at_least_250_words_of_visible_prose() {
        for pair in all_pairs() {
            let text = strip_tags(&content_html(&pair));
            let words = text.split_whitespace().count();
            assert!(
                words >= 250,
                "{} has only {words} words of visible prose",
                pair.path()
            );
        }
    }

    #[test]
    fn every_pair_page_emits_faq_details_for_json_ld() {
        for pair in all_pairs() {
            let content = content_html(&pair);
            let faqs = crate::faq::extract_faqs(&content);
            assert!(
                (2..=3).contains(&faqs.len()),
                "{} has {} FAQs (want 2–3)",
                pair.path(),
                faqs.len()
            );
        }
    }

    #[test]
    fn sibling_links_cover_other_targets_reverse_and_parent() {
        let pairs = all_pairs();
        let html = render_pair_page(&branded(), &audio_meta(), find(&pairs, "wav", "mp3"), &pairs);
        // other targets for the same source
        for sib in ["wav-to-ogg", "wav-to-flac", "wav-to-m4a"] {
            assert!(
                html.contains(&format!(r#"href="/tools/audio-convert/{sib}/""#)),
                "sibling {sib} linked"
            );
        }
        // the reverse direction exists (mp3 is a valid source, wav a valid target)
        assert!(html.contains(r#"href="/tools/audio-convert/mp3-to-wav/""#));
        // and the parent converter
        assert!(html.contains(r#"href="/tools/audio-convert/""#));
        // a source-only format (opus) has no reverse page to link
        let opus = render_pair_page(&branded(), &audio_meta(), find(&pairs, "opus", "mp3"), &pairs);
        assert!(!opus.contains("mp3-to-opus"));
    }

    #[test]
    fn direction_copy_matches_the_parent_tools_documented_behavior() {
        let pairs = all_pairs();
        // lossy→lossless cannot restore quality
        let mp3_flac = content_html(find(&pairs, "mp3", "flac"));
        assert!(mp3_flac.contains("No quality is restored"));
        // lossless→lossy explains the bitrate choice
        let wav_mp3 = content_html(find(&pairs, "wav", "mp3"));
        assert!(wav_mp3.contains("32 and 320 kbps"));
        assert!(wav_mp3.contains("192 kbps"));
        // lossless targets: no bitrate to choose
        let wav_flac = content_html(find(&pairs, "wav", "flac"));
        assert!(wav_flac.contains("no bitrate to choose"));
        // every audio pair mentions the dropped album art (the tool passes -vn)
        for pair in pairs.iter().filter(|p| p.parent == "audio-convert") {
            assert!(
                content_html(pair).contains("album art is dropped"),
                "{} album-art caveat",
                pair.path()
            );
        }
    }

    #[test]
    fn pairs_json_lists_path_and_title() {
        let pairs = all_pairs();
        let refs: Vec<&PairSpec> = pairs.iter().collect();
        let v: serde_json::Value = serde_json::from_str(&pairs_json(&branded(), &refs)).unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 50);
        let wav_mp3 = arr
            .iter()
            .find(|e| e["path"] == "audio-convert/wav-to-mp3")
            .expect("wav-to-mp3 entry");
        assert_eq!(
            wav_mp3["title"],
            "Convert WAV to MP3 Online — Free & Private — gizza.ai"
        );
        assert!(arr.iter().any(|e| e["path"] == "image-convert/png-to-webp"));
        // every path is parent-slash-pair shaped
        for e in arr {
            let path = e["path"].as_str().unwrap();
            assert!(
                path.starts_with("audio-convert/") || path.starts_with("image-convert/"),
                "{path}"
            );
            assert!(path.contains("-to-"), "{path}");
        }
    }

    #[test]
    fn pair_markdown_has_cta_cli_and_parent_docs_link() {
        let pairs = all_pairs();
        let md = pair_markdown(&branded(), &audio_meta(), find(&pairs, "wav", "mp3"));
        assert!(md.starts_with("# Convert WAV to MP3\n"));
        assert!(md.contains("https://gizza.ai/tools/audio-convert/?format=mp3"));
        assert!(md.contains(
            "`gizza tool audio-convert 'url=https://example.com/input.wav' 'format=mp3'`"
        ));
        assert!(md.contains("https://gizza.ai/tools/audio-convert/index.md"));

        let md = pair_markdown(&branded(), &image_meta(), find(&pairs, "png", "webp"));
        assert!(md.contains(
            "`gizza tool image-convert 'url=https://example.com/input.png' 'format=webp' 'quality=85'`"
        ));
    }

    #[test]
    fn pair_links_for_parent_builds_the_popular_conversions_list() {
        let pairs = all_pairs();
        let links = pair_links_for_parent(&pairs, "audio-convert");
        assert_eq!(links.len(), 35);
        assert!(links
            .iter()
            .any(|l| l.label == "WAV to MP3" && l.href == "/tools/audio-convert/wav-to-mp3/"));
        let links = pair_links_for_parent(&pairs, "image-convert");
        assert_eq!(links.len(), 15);
        assert!(links
            .iter()
            .any(|l| l.label == "JPG to WebP" && l.href == "/tools/image-convert/jpg-to-webp/"));
        assert!(pair_links_for_parent(&pairs, "calculator").is_empty());
    }

    #[test]
    fn default_config_renders_a_generic_unbranded_pair_page() {
        let pairs = all_pairs();
        let pair = find(&pairs, "wav", "mp3");
        let html = render_pair_page(&SiteConfig::default(), &audio_meta(), pair, &pairs);
        assert!(html.contains("<title>Convert WAV to MP3 Online — Free &amp; Private</title>"));
        assert!(!html.contains(r#"<link rel="canonical""#), "no canonical when unbranded");
        assert!(!html.contains("og:image"), "no og:image when unbranded");
        assert!(!html.contains("BreadcrumbList"), "no BreadcrumbList JSON-LD when unbranded");
        assert!(html.contains(r#"class="tool-nav""#), "generic header rendered");
        assert!(html.contains(r#"class="tool-footer""#), "generic footer rendered");
        assert!(!html.contains("gizza"), "fully generic — no brand leaks");

        let md = pair_markdown(&SiteConfig::default(), &audio_meta(), pair);
        assert!(
            md.contains("- **Web (MP3 preselected):** /tools/audio-convert/?format=mp3\n"),
            "web link stays relative when unbranded"
        );
        assert!(
            md.contains("- **Parent tool docs:** /tools/audio-convert/index.md\n"),
            "parent docs link stays relative when unbranded"
        );
    }
}
