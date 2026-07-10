//! gizza-ai/pin-image-resizer core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Resizes and crops an image to one of Pinterest's recommended pin formats:
//! - `standard` 1000×1500 (2:3) — the recommended pin, performs best in-feed.
//! - `square`   1000×1000 (1:1).
//! - `tall`     1000×2100 — long / infographic pin (Pinterest crops anything
//!    taller than ~1:2.1 in the feed, so this is the practical max).
//! - `story`    1080×1920 (9:16) — idea / story pin, full-screen mobile.
//!
//! Three fit modes decide how a differently-shaped source lands in the pin box:
//! `cover` (default) scales up and crops the overflow (no distortion, no bars);
//! `contain` scales down inside the box and pads the rest with a background
//! colour; `stretch` forces the exact box, distorting if the ratio differs.

/// A Pinterest pin format and its recommended pixel dimensions.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub w: u32,
    pub h: u32,
}

/// Resolve a preset name (case-insensitive, with common aliases) to its
/// recommended `(width, height)` in pixels. An empty string is the default
/// `standard` pin.
pub fn preset_dims(s: &str) -> Result<Preset, String> {
    let norm: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    match norm.as_str() {
        "" | "standard" | "23" | "pin" => Ok(Preset { name: "standard", w: 1000, h: 1500 }),
        "square" | "11" => Ok(Preset { name: "square", w: 1000, h: 1000 }),
        "tall" | "long" | "infographic" => Ok(Preset { name: "tall", w: 1000, h: 2100 }),
        "story" | "idea" | "916" => Ok(Preset { name: "story", w: 1080, h: 1920 }),
        _ => Err(format!(
            "invalid preset {s:?}; expected one of: standard, square, tall, story"
        )),
    }
}

/// How the source image is fitted into the pin box.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Fit {
    /// Scale to fill the box, crop the overflow (no distortion, no bars).
    Cover,
    /// Scale to fit inside the box, pad the remainder with a background colour.
    Contain,
    /// Force the exact box, distorting the image if the ratio differs.
    Stretch,
}

pub fn parse_fit(s: Option<&str>) -> Result<Fit, String> {
    match s.unwrap_or("cover").trim().to_ascii_lowercase().as_str() {
        "" | "cover" => Ok(Fit::Cover),
        "contain" | "pad" | "fit" => Ok(Fit::Contain),
        "stretch" => Ok(Fit::Stretch),
        other => Err(format!("invalid fit {other:?}; expected cover|contain|stretch")),
    }
}

/// Which part of the image to keep when `cover` crops the overflow.
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

/// Normalise a colour for ffmpeg: `#rgb[a]` expands, `#rrggbb[aa]` →
/// `0xrrggbb[aa]`; named colours
/// (white, black, …) pass through unchanged. Rejects anything with characters
/// that could break the filtergraph so the argv stays safe.
fn normalize_color(c: &str) -> Result<String, String> {
    let t = c.trim();
    if t.is_empty() {
        return Ok("white".to_string());
    }
    let body = t.strip_prefix('#').unwrap_or(t);
    let out = if t.starts_with('#') {
        if !body.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "invalid hex colour {t:?}; use #rgb, #rgba, #rrggbb, or #rrggbbaa"
            ));
        }
        let expanded = match body.len() {
            3 | 4 => body.chars().flat_map(|ch| [ch, ch]).collect::<String>(),
            6 | 8 => body.to_string(),
            _ => {
                return Err(format!(
                    "invalid hex colour {t:?}; use #rgb, #rgba, #rrggbb, or #rrggbbaa"
                ))
            }
        };
        format!("0x{expanded}")
    } else {
        // A named colour like "white"/"black"; keep it strictly alphanumeric so
        // it can't inject filter syntax (':', ',', quotes, spaces).
        if !t.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!(
                "invalid colour {t:?}; use a hex value (#ffffff) or a plain colour name"
            ));
        }
        t.to_ascii_lowercase()
    };
    Ok(out)
}

/// Build the ffmpeg video-filter string for the chosen box, fit, gravity and
/// background colour.
fn build_vf(w: u32, h: u32, fit: Fit, gravity: Gravity, bg: &str) -> String {
    match fit {
        Fit::Stretch => format!("scale={w}:{h}"),
        Fit::Contain => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:{bg}"
        ),
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

/// Build the ffmpeg argv (no leading "ffmpeg") for an input/output file pair.
pub fn build_argv(in_name: &str, out_name: &str, vf: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        vf.into(),
        out_name.into(),
    ]
}

/// Validate the params and return `(argv, out_name)` for an input file.
/// `out_name` keeps the input extension. Used by both the chat block and the
/// web page (file in → file out).
pub fn plan(
    in_name: &str,
    preset: &str,
    fit: Option<&str>,
    gravity: Option<&str>,
    background: &str,
) -> Result<(Vec<String>, String), String> {
    let p = preset_dims(preset)?;
    let fit = parse_fit(fit)?;
    let gravity = parse_gravity(gravity)?;
    let bg = normalize_color(background)?;
    let vf = build_vf(p.w, p.h, fit, gravity, &bg);
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("png");
    let out_name = format!("pin-{}x{}.{ext}", p.w, p.h);
    Ok((build_argv(in_name, &out_name, &vf), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_resolve_and_alias() {
        assert_eq!(preset_dims("standard").unwrap(), Preset { name: "standard", w: 1000, h: 1500 });
        assert_eq!(preset_dims("").unwrap().name, "standard");
        assert_eq!(preset_dims("2:3").unwrap().h, 1500);
        assert_eq!(preset_dims("Square").unwrap(), Preset { name: "square", w: 1000, h: 1000 });
        assert_eq!(preset_dims("long").unwrap(), Preset { name: "tall", w: 1000, h: 2100 });
        assert_eq!(preset_dims("idea").unwrap(), Preset { name: "story", w: 1080, h: 1920 });
    }

    #[test]
    fn preset_rejects_unknown() {
        assert!(preset_dims("banner").is_err());
        assert!(preset_dims("4:5").is_err());
    }

    #[test]
    fn fit_and_gravity_parse() {
        assert_eq!(parse_fit(None).unwrap(), Fit::Cover);
        assert_eq!(parse_fit(Some("contain")).unwrap(), Fit::Contain);
        assert_eq!(parse_fit(Some("stretch")).unwrap(), Fit::Stretch);
        assert!(parse_fit(Some("squish")).is_err());
        assert_eq!(parse_gravity(None).unwrap(), Gravity::Center);
        assert_eq!(parse_gravity(Some("top")).unwrap(), Gravity::Top);
        assert!(parse_gravity(Some("left")).is_err());
    }

    #[test]
    fn color_normalizes_hex_and_names() {
        assert_eq!(normalize_color("#ffffff").unwrap(), "0xffffff");
        assert_eq!(normalize_color("#f00").unwrap(), "0xff0000");
        assert_eq!(normalize_color("#0f08").unwrap(), "0x00ff0088");
        assert_eq!(normalize_color("#000000ff").unwrap(), "0x000000ff");
        assert_eq!(normalize_color("white").unwrap(), "white");
        assert_eq!(normalize_color("").unwrap(), "white");
        assert!(normalize_color("#12").is_err());
        assert!(normalize_color("rgb(1,2,3)").is_err());
        assert!(normalize_color("red;drop").is_err());
    }

    #[test]
    fn cover_uses_scale_increase_and_center_crop() {
        let (argv, out) = plan("in.jpg", "standard", Some("cover"), None, "white").unwrap();
        assert_eq!(out, "pin-1000x1500.jpg");
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(
            vf,
            "scale=1000:1500:force_original_aspect_ratio=increase,crop=1000:1500"
        );
    }

    #[test]
    fn cover_top_gravity_pins_crop_to_top() {
        let (argv, _) = plan("in.png", "square", Some("cover"), Some("top"), "white").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert!(vf.ends_with("crop=1000:1000:(iw-1000)/2:0"), "got {vf}");
    }

    #[test]
    fn contain_pads_with_normalized_color() {
        let (argv, _) = plan("in.png", "story", Some("contain"), None, "#ff0000").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(
            vf,
            "scale=1080:1920:force_original_aspect_ratio=decrease,pad=1080:1920:(ow-iw)/2:(oh-ih)/2:0xff0000"
        );
    }

    #[test]
    fn stretch_is_plain_scale() {
        let (argv, _) = plan("in.png", "standard", Some("stretch"), None, "white").unwrap();
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=1000:1500");
    }

    #[test]
    fn plan_rejects_bad_preset() {
        assert!(plan("in.png", "widescreen", None, None, "white").is_err());
    }
}
