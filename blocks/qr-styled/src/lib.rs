//! gizza-ai/qr-styled — generate a styled QR code SVG: custom colours, an
//! optional linear/radial gradient body, square / rounded / dot module shapes,
//! styled finder "eyes", and an optional embedded centre logo.
//!
//! Pure-Rust (`qrcode`; the SVG is hand-built), so it runs on ALL backends incl.
//! the chat Service Worker. The image is wrapped as an `image/svg+xml` data-URL
//! envelope. Surfaces: chat + CLI (image-bytes output → no page, like the
//! qr-code-generator / wifi-qr tools). Distinct from qr-code-generator, which
//! emits a plain solid-colour code with no gradients / module shapes / styled
//! eyes / logo.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_qr_styled_core::{
    generate, Ecc, EyeShape, Gradient, ModuleShape, Style,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    data: String,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default = "default_margin")]
    margin: u32,
    #[serde(default = "default_ecc")]
    error_correction: String,
    #[serde(default = "default_fg")]
    fg_color: String,
    #[serde(default = "default_bg")]
    bg_color: String,
    #[serde(default = "default_gradient")]
    gradient: String,
    #[serde(default = "default_grad_color")]
    gradient_color: String,
    #[serde(default = "default_grad_angle")]
    gradient_angle: f64,
    #[serde(default = "default_module_shape")]
    module_shape: String,
    #[serde(default = "default_eye_shape")]
    eye_shape: String,
    #[serde(default)]
    eye_color: String,
    #[serde(default)]
    logo: String,
    #[serde(default = "default_logo_size")]
    logo_size: f64,
}
fn default_size() -> u32 {
    512
}
fn default_margin() -> u32 {
    4
}
fn default_ecc() -> String {
    "M".to_string()
}
fn default_fg() -> String {
    "#000000".to_string()
}
fn default_bg() -> String {
    "#ffffff".to_string()
}
fn default_gradient() -> String {
    "none".to_string()
}
fn default_grad_color() -> String {
    "#000000".to_string()
}
fn default_grad_angle() -> f64 {
    45.0
}
fn default_module_shape() -> String {
    "square".to_string()
}
fn default_eye_shape() -> String {
    "square".to_string()
}
fn default_logo_size() -> f64 {
    0.2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("Text, URL, or any payload to encode into the QR code."),
        )
        .param(
            Param::integer("size")
                .default(512)
                .min(64.0)
                .max(4096.0)
                .describe("Output SVG width/height in pixels (64-4096). The code is vector, so it stays crisp at any size."),
        )
        .param(
            Param::integer("margin")
                .default(4)
                .min(0.0)
                .max(32.0)
                .describe("Quiet-zone width in modules around the code (0-32). 4 is the QR-spec minimum for reliable scanning."),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"]).default("M").describe(
                "Error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code. Forced to H when a logo is embedded.",
            ),
        )
        .param(
            Param::string("fg_color")
                .default("#000000")
                .describe("Foreground (module) colour as #rgb or #rrggbb hex."),
        )
        .param(
            Param::string("bg_color")
                .default("#ffffff")
                .describe("Background colour as #rgb or #rrggbb hex, or 'transparent' for no background."),
        )
        .param(
            Param::enumv("gradient", ["none", "linear", "radial"]).default("none").describe(
                "Body gradient: none (solid fg_color, default), linear (fg_color to gradient_color along gradient_angle), or radial (fg_color at centre to gradient_color at the edge).",
            ),
        )
        .param(
            Param::string("gradient_color")
                .default("#000000")
                .describe("Second gradient stop colour as #rgb or #rrggbb hex (used when gradient is linear or radial)."),
        )
        .param(
            Param::number("gradient_angle")
                .default(45.0)
                .min(0.0)
                .max(360.0)
                .describe("Linear gradient direction in degrees (0-360; used when gradient=linear)."),
        )
        .param(
            Param::enumv("module_shape", ["square", "rounded", "dots"]).default("square").describe(
                "Shape of each data module: square (default), rounded (rounded-corner squares), or dots (circles).",
            ),
        )
        .param(
            Param::enumv("eye_shape", ["square", "rounded", "circle"]).default("square").describe(
                "Shape of the three finder 'eyes': square (default), rounded (rounded corners), or circle.",
            ),
        )
        .param(
            Param::string("eye_color")
                .default("")
                .describe("Eye (finder) colour as #rgb or #rrggbb hex. Leave empty to match fg_color."),
        )
        .param(
            Param::string("logo")
                .default("")
                .describe("Optional centre logo as a data:image/... URI (e.g. data:image/png;base64,...). Embedded verbatim with a knockout behind it; error correction is forced to H. No network fetch."),
        )
        .param(
            Param::number("logo_size")
                .default(0.2)
                .min(0.1)
                .max(0.35)
                .describe("Logo edge as a fraction of the code width (0.1-0.35; used only when a logo is supplied)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct QrStyled;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qr-styled",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a styled QR code SVG",
    skill(
        description = "Generate a styled QR code as a scalable SVG image from any text or URL (data). Style it with fg_color and bg_color (#rgb/#rrggbb hex; bg_color also accepts 'transparent'); add a body gradient (gradient none/linear/radial with gradient_color and gradient_angle); pick a module_shape (square, rounded, or dots) and an eye_shape (square, rounded, or circle) with an optional separate eye_color; and embed a centre logo (logo as a data:image/... URI, sized by logo_size 0.1-0.35 — error correction is forced to H so it still scans). error_correction is L/M/Q/H (default M), margin sets the quiet zone in modules, and size sets the SVG pixel edge. Returns an image. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl QrStyled {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("qr-styled")?;
    let ecc = Ecc::parse(&args.error_correction).map_err(SkillError::InvalidArgs)?;
    let gradient = Gradient::parse(&args.gradient).map_err(SkillError::InvalidArgs)?;
    let module_shape = ModuleShape::parse(&args.module_shape).map_err(SkillError::InvalidArgs)?;
    let eye_shape = EyeShape::parse(&args.eye_shape).map_err(SkillError::InvalidArgs)?;
    let style = Style {
        size: args.size,
        margin: args.margin,
        ecc,
        fg_color: args.fg_color,
        bg_color: args.bg_color,
        gradient,
        gradient_color: args.gradient_color,
        gradient_angle: args.gradient_angle,
        module_shape,
        eye_shape,
        eye_color: args.eye_color,
        logo: args.logo,
        logo_size: args.logo_size,
    };
    let g = generate(&args.data, &style).map_err(SkillError::InvalidArgs)?;
    let n = g.bytes.len();
    build_media_envelope(
        &g.bytes,
        "image/svg+xml",
        "qr-styled.svg".to_string(),
        format!("Styled QR code (SVG image, {n} bytes, EC {:?}) encoding: {}", g.ecc, g.payload),
        MAX_OUTPUT_BYTES,
    )
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
                    "data": { "type": "string", "description": "Text, URL, or any payload to encode into the QR code." },
                    "size": { "type": "integer", "default": 512, "minimum": 64, "maximum": 4096, "description": "Output SVG width/height in pixels (64-4096). The code is vector, so it stays crisp at any size." },
                    "margin": { "type": "integer", "default": 4, "minimum": 0, "maximum": 32, "description": "Quiet-zone width in modules around the code (0-32). 4 is the QR-spec minimum for reliable scanning." },
                    "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "default": "M", "description": "Error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code. Forced to H when a logo is embedded." },
                    "fg_color": { "type": "string", "default": "#000000", "description": "Foreground (module) colour as #rgb or #rrggbb hex." },
                    "bg_color": { "type": "string", "default": "#ffffff", "description": "Background colour as #rgb or #rrggbb hex, or 'transparent' for no background." },
                    "gradient": { "type": "string", "enum": ["none", "linear", "radial"], "default": "none", "description": "Body gradient: none (solid fg_color, default), linear (fg_color to gradient_color along gradient_angle), or radial (fg_color at centre to gradient_color at the edge)." },
                    "gradient_color": { "type": "string", "default": "#000000", "description": "Second gradient stop colour as #rgb or #rrggbb hex (used when gradient is linear or radial)." },
                    "gradient_angle": { "type": "number", "default": 45.0, "minimum": 0, "maximum": 360, "description": "Linear gradient direction in degrees (0-360; used when gradient=linear)." },
                    "module_shape": { "type": "string", "enum": ["square", "rounded", "dots"], "default": "square", "description": "Shape of each data module: square (default), rounded (rounded-corner squares), or dots (circles)." },
                    "eye_shape": { "type": "string", "enum": ["square", "rounded", "circle"], "default": "square", "description": "Shape of the three finder 'eyes': square (default), rounded (rounded corners), or circle." },
                    "eye_color": { "type": "string", "default": "", "description": "Eye (finder) colour as #rgb or #rrggbb hex. Leave empty to match fg_color." },
                    "logo": { "type": "string", "default": "", "description": "Optional centre logo as a data:image/... URI (e.g. data:image/png;base64,...). Embedded verbatim with a knockout behind it; error correction is forced to H. No network fetch." },
                    "logo_size": { "type": "number", "default": 0.2, "minimum": 0.1, "maximum": 0.35, "description": "Logo edge as a fraction of the code width (0.1-0.35; used only when a logo is supplied)." }
                },
                "additionalProperties": false,
                "required": ["data"]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
