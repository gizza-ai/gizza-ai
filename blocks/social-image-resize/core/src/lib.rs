//! social-image-resize core — pure ffmpeg argv construction shared by the chat block + page.
//! No wafer/wasm-bindgen deps.
//!
//! Resizes/crops one image to a named social-platform preset. The current gizza
//! model emits one output file, so "all sizes at once" is represented as quick
//! preset chips rather than a multi-file ZIP.

/// A social image target and its pixel dimensions.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Target {
    pub name: &'static str,
    pub w: u32,
    pub h: u32,
}

pub const TARGETS: &[Target] = &[
    Target { name: "instagram-square", w: 1080, h: 1080 },
    Target { name: "instagram-portrait", w: 1080, h: 1350 },
    Target { name: "instagram-story", w: 1080, h: 1920 },
    Target { name: "facebook-post", w: 1200, h: 630 },
    Target { name: "facebook-cover", w: 820, h: 312 },
    Target { name: "twitter-post", w: 1600, h: 900 },
    Target { name: "twitter-header", w: 1500, h: 500 },
    Target { name: "linkedin-post", w: 1200, h: 627 },
    Target { name: "linkedin-cover", w: 1584, h: 396 },
    Target { name: "youtube-thumbnail", w: 1280, h: 720 },
    Target { name: "pinterest-pin", w: 1000, h: 1500 },
    Target { name: "tiktok-video", w: 1080, h: 1920 },
];

/// Resolve a target name (case-insensitive; `_` and spaces allowed) to pixels.
/// Empty defaults to `instagram-square`.
pub fn target_dims(s: &str) -> Result<Target, String> {
    let norm = s.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    let norm = norm.as_str();
    match norm {
        "" | "instagram-square" | "ig-square" | "square" => Ok(TARGETS[0]),
        "instagram-portrait" | "ig-portrait" | "instagram-post-portrait" => Ok(TARGETS[1]),
        "instagram-story" | "instagram-reel" | "ig-story" | "story" | "reel" => Ok(TARGETS[2]),
        "facebook-post" | "fb-post" | "open-graph" | "og" => Ok(TARGETS[3]),
        "facebook-cover" | "fb-cover" => Ok(TARGETS[4]),
        "twitter-post" | "x-post" | "twitter-card" | "x-card" => Ok(TARGETS[5]),
        "twitter-header" | "x-header" => Ok(TARGETS[6]),
        "linkedin-post" => Ok(TARGETS[7]),
        "linkedin-cover" => Ok(TARGETS[8]),
        "youtube-thumbnail" | "yt-thumbnail" | "thumbnail" => Ok(TARGETS[9]),
        "pinterest-pin" | "pin" => Ok(TARGETS[10]),
        "tiktok-video" | "tiktok" => Ok(TARGETS[11]),
        _ => Err(format!(
            "invalid target {s:?}; expected one of: instagram-square, instagram-portrait, instagram-story, facebook-post, facebook-cover, twitter-post, twitter-header, linkedin-post, linkedin-cover, youtube-thumbnail, pinterest-pin, tiktok-video"
        )),
    }
}

/// How the source image is fitted into the output box.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Fit {
    Cover,
    Contain,
    Stretch,
}

pub fn parse_fit(s: Option<&str>) -> Result<Fit, String> {
    match s.unwrap_or("cover").trim().to_ascii_lowercase().as_str() {
        "" | "cover" | "crop" | "fill" => Ok(Fit::Cover),
        "contain" | "pad" | "fit" => Ok(Fit::Contain),
        "stretch" => Ok(Fit::Stretch),
        other => Err(format!("invalid fit {other:?}; expected cover|contain|stretch")),
    }
}

/// Which part of the image to keep when `cover` crops overflow.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Gravity {
    Center,
    Top,
    Bottom,
}

pub fn parse_gravity(s: Option<&str>) -> Result<Gravity, String> {
    match s.unwrap_or("center").trim().to_ascii_lowercase().as_str() {
        "" | "center" | "centre" | "middle" => Ok(Gravity::Center),
        "top" | "north" => Ok(Gravity::Top),
        "bottom" | "south" => Ok(Gravity::Bottom),
        other => Err(format!("invalid gravity {other:?}; expected center|top|bottom")),
    }
}

/// Normalize a safe ffmpeg colour. Hex colours become `0x...`; plain colour
/// names must be alphanumeric so filter syntax cannot be injected.
fn normalize_color(c: &str) -> Result<String, String> {
    let t = c.trim();
    if t.is_empty() {
        return Ok("white".to_string());
    }
    let body = t.strip_prefix('#').unwrap_or(t);
    if t.starts_with('#') {
        if !body.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("invalid hex colour {t:?}; use #rgb, #rgba, #rrggbb, or #rrggbbaa"));
        }
        let expanded = match body.len() {
            3 | 4 => body.chars().flat_map(|ch| [ch, ch]).collect::<String>(),
            6 | 8 => body.to_string(),
            _ => return Err(format!("invalid hex colour {t:?}; use #rgb, #rgba, #rrggbb, or #rrggbbaa")),
        };
        Ok(format!("0x{expanded}"))
    } else if t.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(t.to_ascii_lowercase())
    } else {
        Err(format!("invalid colour {t:?}; use a hex value (#ffffff) or a plain colour name"))
    }
}

fn build_vf(w: u32, h: u32, fit: Fit, gravity: Gravity, bg: &str) -> String {
    match fit {
        Fit::Stretch => format!("scale={w}:{h}"),
        Fit::Contain => format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:{bg}"),
        Fit::Cover => {
            let crop = match gravity {
                Gravity::Center => format!("crop={w}:{h}"),
                Gravity::Top => format!("crop={w}:{h}:(iw-{w})/2:0"),
                Gravity::Bottom => format!("crop={w}:{h}:(iw-{w})/2:(ih-{h})"),
            };
            format!("scale={w}:{h}:force_original_aspect_ratio=increase,{crop}")
        }
    }
}

pub fn build_argv(in_name: &str, out_name: &str, vf: &str) -> Vec<String> {
    vec!["-i".into(), in_name.into(), "-vf".into(), vf.into(), out_name.into()]
}

/// Validate params and return `(argv, out_name)` for an input image. Output keeps
/// the input extension.
pub fn plan(
    in_name: &str,
    target: &str,
    fit: Option<&str>,
    gravity: Option<&str>,
    background: &str,
) -> Result<(Vec<String>, String), String> {
    let t = target_dims(target)?;
    let fit = parse_fit(fit)?;
    let gravity = parse_gravity(gravity)?;
    let bg = normalize_color(background)?;
    let vf = build_vf(t.w, t.h, fit, gravity, &bg);
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("png");
    let out_name = format!("social-{}-{}x{}.{}", t.name, t.w, t.h, ext);
    Ok((build_argv(in_name, &out_name, &vf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_resolve_with_aliases() {
        assert_eq!(target_dims("").unwrap(), Target { name: "instagram-square", w: 1080, h: 1080 });
        assert_eq!(target_dims("ig story").unwrap(), Target { name: "instagram-story", w: 1080, h: 1920 });
        assert_eq!(target_dims("x-header").unwrap(), Target { name: "twitter-header", w: 1500, h: 500 });
        assert_eq!(target_dims("thumbnail").unwrap(), Target { name: "youtube-thumbnail", w: 1280, h: 720 });
    }

    #[test]
    fn rejects_bad_target_and_fit() {
        assert!(target_dims("snapchat").is_err());
        assert!(parse_fit(Some("squash")).is_err());
        assert!(parse_gravity(Some("left")).is_err());
    }

    #[test]
    fn cover_uses_increase_and_crop() {
        let (argv, out) = plan("in.jpg", "linkedin-post", Some("cover"), Some("top"), "white").unwrap();
        assert_eq!(out, "social-linkedin-post-1200x627.jpg");
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=1200:627:force_original_aspect_ratio=increase,crop=1200:627:(iw-1200)/2:0");
    }

    #[test]
    fn contain_pads_with_short_hex() {
        let (argv, _) = plan("in.png", "facebook-post", Some("contain"), None, "#f00").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=1200:630:force_original_aspect_ratio=decrease,pad=1200:630:(ow-iw)/2:(oh-ih)/2:0xff0000");
    }

    #[test]
    fn stretch_is_plain_scale() {
        let (argv, _) = plan("in.webp", "pinterest-pin", Some("stretch"), None, "white").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=1000:1500");
    }

    #[test]
    fn color_validation_blocks_filter_injection() {
        assert_eq!(normalize_color("#0f08").unwrap(), "0x00ff0088");
        assert_eq!(normalize_color("Black").unwrap(), "black");
        assert!(normalize_color("#12").is_err());
        assert!(normalize_color("red:evil").is_err());
    }
}
