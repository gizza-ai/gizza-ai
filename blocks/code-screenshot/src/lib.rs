//! gizza-ai/code-screenshot — render a syntax-highlighted code snippet into a
//! shareable PNG (carbon-style window chrome + padded background).
//!
//! Input is plain text params (`code` required; `language`, `theme` optional) —
//! NOT a file/url, so there is no `block-utils` Source / attachment lookup. The
//! pure render lives in the `core` crate (syntect + fontdue + png, wasm-safe);
//! this block parses args, calls `render_png`, base64-encodes the PNG, and emits
//! the renderable `{_for_llm, _for_ui}` envelope (like `image-fetch`). There is
//! NO standalone page (image-from-text fits neither page shape) — chat + CLI only.

// The #[wafer_block] macro emits wasm-only registration; the supporting imports
// and the Args type are only used inside that wasm32-gated impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, SkillError, SkillResultExt};
use gizza_ai_code_screenshot_core::{render_png, RenderError};
use serde::Deserialize;
use wafer_sdk::*;

/// Refuse to surface an absurdly large PNG (defensive — a few hundred lines of
/// code render well under this).
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    theme: Option<String>,
}

/// Map a core `RenderError` onto a user-facing `SkillError`.
fn map_render_err(e: RenderError) -> SkillError {
    match e {
        RenderError::EmptyCode => {
            SkillError::InvalidArgs("invalid code-screenshot args: `code` is empty".into())
        }
    }
}

#[cfg(target_arch = "wasm32")]
struct CodeScreenshot;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-screenshot",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a code snippet into a syntax-highlighted PNG",
    skill(
        description = "Render a code snippet into a shareable, syntax-highlighted PNG image (carbon-style window chrome on a padded background). Use this when the user wants a picture/screenshot/carbon image of code rather than the code as text. Returns an image, not text.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "The source code to render. Required." },
                "language": { "type": "string", "description": "Language for syntax highlighting (e.g. \"rust\", \"python\", \"javascript\", \"bash\"). Optional — unknown or omitted falls back to plain text." },
                "theme": { "type": "string", "description": "Color theme name (e.g. \"base16-ocean.dark\", \"Solarized (dark)\", \"InspiredGitHub\"). Optional — defaults to a dark theme." }
            },
            "required": ["code"],
            "additionalProperties": false
        }"#
    )
)]
impl CodeScreenshot {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("code-screenshot")?;

    let (png, width, height) =
        render_png(&args.code, args.language.as_deref(), args.theme.as_deref())
            .map_err(map_render_err)?;

    if png.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "output image",
            bytes: png.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    let bytes = png.len();
    let line_count = args.code.lines().count().max(1);
    let lang = args
        .language
        .as_deref()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .unwrap_or("plaintext");

    let encoded = B64.encode(&png);
    let data_url = format!("data:image/png;base64,{encoded}");
    let env = Envelope {
        for_llm: format!(
            "rendered a {width}x{height} PNG ({bytes} bytes) of {line_count} line(s) of {lang} code"
        ),
        for_ui: ForUi {
            data_url,
            mime: "image/png".to_string(),
            filename: "code.png".to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_code_maps_to_invalid_args() {
        let err = map_render_err(RenderError::EmptyCode);
        assert!(matches!(
            err,
            SkillError::InvalidArgs(ref s) if s.contains("`code` is empty")
        ));
    }

    #[test]
    fn args_parse_with_only_required_code() {
        let a: Args = serde_json::from_str(r#"{"code":"fn main(){}"}"#).unwrap();
        assert_eq!(a.code, "fn main(){}");
        assert!(a.language.is_none());
        assert!(a.theme.is_none());
    }

    #[test]
    fn args_parse_with_language_and_theme() {
        let a: Args =
            serde_json::from_str(r#"{"code":"x=1","language":"python","theme":"InspiredGitHub"}"#)
                .unwrap();
        assert_eq!(a.language.as_deref(), Some("python"));
        assert_eq!(a.theme.as_deref(), Some("InspiredGitHub"));
    }
}
