//! gizza-ai/css-color-converter — chat skill block on the shared tool abstraction.
//! Parses one color in any common CSS or app notation (#hex, rgb(), hsl(), hwb(),
//! oklch(), oklab(), a CSS name, a bare triple, or the 0xAARRGGBB integer form)
//! and prints every other notation for the same color, plus the WCAG contrast it
//! earns on white and black. Chat schema single-sourced from descriptor(); the
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_css_color_converter_core::{convert, render_text, Options, Syntax};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_syntax")]
    syntax: String,
    #[serde(default = "default_precision")]
    precision: f64,
    #[serde(default)]
    uppercase_hex: bool,
}

fn default_syntax() -> String {
    "legacy".to_string()
}
fn default_precision() -> f64 {
    3.0
}

/// Turn the wire args into core [`Options`], clamping precision to the advertised
/// 0–8 range rather than failing on an out-of-range number.
fn options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        syntax: Syntax::parse(&a.syntax)?,
        precision: a.precision.round().clamp(0.0, 8.0) as u32,
        uppercase_hex: a.uppercase_hex,
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The color to convert, in whatever notation you already have it in. Accepted: 3-, 4-, 6- and 8-digit hex with or without the leading # (#3498db, #f00, #3498db80, f00); the 0xAARRGGBB / 0xRRGGBB integer form Flutter, Jetpack Compose and Android use, including the pasted Color(0xFF3498DB) snippet; rgb()/rgba(), hsl()/hsla(), hwb(), oklch() and oklab() in either the legacy comma form or the CSS Color 4 space-and-slash form; a bare 52, 152, 219 triple; any of the 148 CSS color names; and transparent. Hue accepts deg, turn, rad and grad units, and alpha accepts 0-1 or a percentage. A color outside the sRGB gamut (an ambitious oklch(), typically) is clamped to the nearest sRGB color and the result says so."),
        )
        .param(
            Param::enumv("syntax", ["legacy", "modern"])
                .default("legacy")
                .describe("Which CSS function syntax the rgb() and hsl() lines are written in. \"legacy\" (default) is the comma-separated form every browser has supported for years, switching to rgba()/hsla() when the color has alpha: rgba(52, 152, 219, 0.5). \"modern\" is the CSS Color 4 form — space-separated channels with a slash before alpha, and no separate rgba()/hsla() names: rgb(52 152 219 / 0.5). This setting only changes rgb() and hsl(); hwb(), lab(), lch(), oklch(), oklab() and color(display-p3 …) exist only in the space-separated form, and HSV and CMYK are not CSS at all, so they always print comma-separated the way design tools show them."),
        )
        .param(
            Param::integer("precision")
                .default(3)
                .min(0.0)
                .max(8.0)
                .describe("Decimal places kept on the fractional components — HSL/HWB/CMYK percentages, the LAB/LCH/OKLCH/OKLab axes, and alpha. 3 by default, which round-trips oklch() back to the same 8-bit hex; 0 gives the short hsl(204, 70%, 53%) form designers paste. Trailing zeros are trimmed, so a whole number prints without a decimal point either way. The 0-1 fractions in the SwiftUI and display-p3 lines never drop below 3 places, since fewer would change the color. Values above 8 are capped at 8."),
        )
        .param(
            Param::boolean("uppercase_hex")
                .default(false)
                .describe("Print hex digits in upper case: #3498DB, #3498DBFF and 0xFF3498DB instead of the lower-case default. Affects every hex-bearing line — the CSS hex, the 8-digit hex with alpha, the Android #AARRGGBB value and the Flutter/Compose 0xAARRGGBB integer. Purely cosmetic: hex colors are case-insensitive everywhere they are read."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CssColorConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-color-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert one color between hex, rgb, hsl, hwb, lab, lch, oklch, oklab, P3, CMYK and app code",
    skill(
        description = "Convert a single color from any common notation into every other notation for the SAME color. Pass the color in `input`: 3/4/6/8-digit hex (with or without `#`), the `0xAARRGGBB` integer form Flutter/Jetpack Compose/Android use (a pasted `Color(0xFF3498DB)` works too), `rgb()`/`rgba()`, `hsl()`/`hsla()`, `hwb()`, `oklch()`, `oklab()`, a bare `52, 152, 219` triple, any CSS color name, or `transparent`. Hue units (`deg`/`turn`/`rad`/`grad`) and 0-1-or-percentage alpha are understood, and both the legacy comma form and the CSS Color 4 space-and-slash form parse. The result is a plain-text block grouped into CSS (hex, hex+alpha, rgb, hsl, hwb, lab, lch, oklch, oklab, `color(display-p3 …)`, and the exact CSS color name or the nearest one by OKLab distance), Design (HSV/HSB and CMYK), App code (Flutter/Dart, Jetpack Compose, SwiftUI, Android XML `#AARRGGBB`, and the signed ARGB int) and Contrast (WCAG 2.1 ratio against white and against black, each with the grade it earns: AAA, AA, AA large text only, or fails WCAG). `syntax` switches rgb()/hsl() between the legacy comma form and the modern space-and-slash form, `precision` sets 0-8 decimals on the fractional components, and `uppercase_hex` prints hex digits in upper case. Channels are quantized to 8-bit sRGB once, up front, so every notation names exactly the same renderable color; a color outside the sRGB gamut is clamped to the nearest one and the output says so. Alpha is preserved throughout, including the CSS 8-digit hex (alpha LAST) versus the `0xAARRGGBB` integer (alpha FIRST). Pure text in, text out: no I/O and no clock, so the same input always gives the same output.",
        parameters = schema_json()
    ),
)]
impl CssColorConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "css-color-converter", |a: Args| {
            let opts = options(&a).map_err(SkillError::InvalidArgs)?;
            let converted = convert(&a.input, &opts).map_err(SkillError::InvalidArgs)?;
            Ok(render_text(&converted))
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The color to convert, in whatever notation you already have it in. Accepted: 3-, 4-, 6- and 8-digit hex with or without the leading # (#3498db, #f00, #3498db80, f00); the 0xAARRGGBB / 0xRRGGBB integer form Flutter, Jetpack Compose and Android use, including the pasted Color(0xFF3498DB) snippet; rgb()/rgba(), hsl()/hsla(), hwb(), oklch() and oklab() in either the legacy comma form or the CSS Color 4 space-and-slash form; a bare 52, 152, 219 triple; any of the 148 CSS color names; and transparent. Hue accepts deg, turn, rad and grad units, and alpha accepts 0-1 or a percentage. A color outside the sRGB gamut (an ambitious oklch(), typically) is clamped to the nearest sRGB color and the result says so." },
                    "syntax": { "type": "string", "enum": ["legacy", "modern"], "default": "legacy", "description": "Which CSS function syntax the rgb() and hsl() lines are written in. \"legacy\" (default) is the comma-separated form every browser has supported for years, switching to rgba()/hsla() when the color has alpha: rgba(52, 152, 219, 0.5). \"modern\" is the CSS Color 4 form — space-separated channels with a slash before alpha, and no separate rgba()/hsla() names: rgb(52 152 219 / 0.5). This setting only changes rgb() and hsl(); hwb(), lab(), lch(), oklch(), oklab() and color(display-p3 …) exist only in the space-separated form, and HSV and CMYK are not CSS at all, so they always print comma-separated the way design tools show them." },
                    "precision": { "type": "integer", "default": 3, "minimum": 0, "maximum": 8, "description": "Decimal places kept on the fractional components — HSL/HWB/CMYK percentages, the LAB/LCH/OKLCH/OKLab axes, and alpha. 3 by default, which round-trips oklch() back to the same 8-bit hex; 0 gives the short hsl(204, 70%, 53%) form designers paste. Trailing zeros are trimmed, so a whole number prints without a decimal point either way. The 0-1 fractions in the SwiftUI and display-p3 lines never drop below 3 places, since fewer would change the color. Values above 8 are capped at 8." },
                    "uppercase_hex": { "type": "boolean", "default": false, "description": "Print hex digits in upper case: #3498DB, #3498DBFF and 0xFF3498DB instead of the lower-case default. Affects every hex-bearing line — the CSS hex, the 8-digit hex with alpha, the Android #AARRGGBB value and the Flutter/Compose 0xAARRGGBB integer. Purely cosmetic: hex colors are case-insensitive everywhere they are read." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_map_onto_core_options() {
        let a: Args = serde_json::from_str(r##"{"input":"#3498db"}"##).unwrap();
        let o = options(&a).unwrap();
        assert_eq!(o.syntax, Syntax::Legacy);
        assert_eq!(o.precision, 3);
        assert!(!o.uppercase_hex);

        // Precision is capped at the advertised maximum instead of erroring.
        let a: Args =
            serde_json::from_str(r##"{"input":"#3498db","precision":42,"syntax":"modern"}"##)
                .unwrap();
        let o = options(&a).unwrap();
        assert_eq!(o.precision, 8);
        assert_eq!(o.syntax, Syntax::Modern);

        let a: Args = serde_json::from_str(r##"{"input":"#3498db","syntax":"bogus"}"##).unwrap();
        assert!(options(&a).unwrap_err().contains("unknown syntax"));
    }

    #[test]
    fn handler_output_is_the_rendered_text_block() {
        let a: Args = serde_json::from_str(r##"{"input":"#3498db"}"##).unwrap();
        let text = render_text(&convert(&a.input, &options(&a).unwrap()).unwrap());
        assert!(
            text.starts_with("CSS\n  HEX                 #3498db"),
            "got:\n{text}"
        );
        assert!(text.contains("  RGB                 rgb(52, 152, 219)"));
        assert!(text.contains("  Flutter / Dart      Color(0xff3498db)"));
    }
}
