//! gizza-ai/qr-code-generator — generate a QR code (SVG or PNG) from text, a
//! URL, Wi-Fi credentials, or a contact card.
//!
//! Pure-Rust (`qrcode` + `image`), so it runs on ALL backends incl. the chat
//! Service Worker. The image is wrapped as an `image/svg+xml` or `image/png`
//! data-URL envelope. Surfaces: chat + CLI (image-bytes output → no page, like
//! the chart / wifi-qr tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_qr_code_generator_core::{generate, Content, Ecc, Fields, Format};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default = "default_content")]
    content: String,
    #[serde(default)]
    data: String,
    // wifi
    #[serde(default)]
    ssid: String,
    #[serde(default)]
    password: String,
    #[serde(default = "default_security")]
    security: String,
    #[serde(default)]
    hidden: bool,
    // contact
    #[serde(default)]
    name: String,
    #[serde(default)]
    phone: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    organization: String,
    #[serde(default)]
    url: String,
    // email / sms
    #[serde(default)]
    subject: String,
    #[serde(default)]
    body: String,
    // geo
    #[serde(default)]
    latitude: String,
    #[serde(default)]
    longitude: String,
    // rendering
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_ecc")]
    error_correction: String,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default = "default_dark")]
    dark_color: String,
    #[serde(default = "default_light")]
    light_color: String,
}
fn default_content() -> String {
    "text".to_string()
}
fn default_security() -> String {
    "WPA".to_string()
}
fn default_format() -> String {
    "svg".to_string()
}
fn default_ecc() -> String {
    "M".to_string()
}
fn default_size() -> u32 {
    512
}
fn default_dark() -> String {
    "#000000".to_string()
}
fn default_light() -> String {
    "#ffffff".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("content", ["text", "url", "wifi", "contact", "email", "sms", "phone", "geo"]).default("text").describe(
                "What to encode: text (any string), url (a link; a scheme is added if missing), wifi (join code from ssid/password/security), contact (a vCard from name/phone/email), email (a mailto: from email/subject/body), sms (an SMSTO: from phone/body), phone (a tel: from phone), or geo (a geo: from latitude/longitude).",
            ),
        )
        .param(Param::string("data").describe(
            "The text or URL to encode (used when content is text or url).",
        ))
        .param(Param::string("ssid").describe("Wi-Fi network name (content=wifi)."))
        .param(Param::string("password").describe(
            "Wi-Fi password (content=wifi; required for WPA/WEP, omit for nopass).",
        ))
        .param(
            Param::enumv("security", ["WPA", "WEP", "nopass"]).default("WPA").describe(
                "Wi-Fi security type (content=wifi): WPA (default, covers WPA/WPA2/WPA3), WEP, or nopass (open).",
            ),
        )
        .param(
            Param::boolean("hidden")
                .default(false)
                .describe("Set true if the Wi-Fi SSID is hidden (content=wifi)."),
        )
        .param(Param::string("name").describe("Contact full name (content=contact)."))
        .param(Param::string("phone").describe("Phone number — contact (content=contact), recipient (content=sms), or number to dial (content=phone)."))
        .param(Param::string("email").describe("Email address — contact (content=contact) or recipient (content=email)."))
        .param(Param::string("organization").describe("Contact organization (content=contact)."))
        .param(Param::string("url").describe("Contact website URL (content=contact)."))
        .param(Param::string("subject").describe("Email subject line (content=email)."))
        .param(Param::string("body").describe("Message body (content=email) or SMS text (content=sms)."))
        .param(Param::string("latitude").describe("Location latitude in decimal degrees (content=geo)."))
        .param(Param::string("longitude").describe("Location longitude in decimal degrees (content=geo)."))
        .param(
            Param::enumv("format", ["svg", "png"]).default("svg").describe(
                "Output image format: svg (scalable, default) or png (raster, sized by the size param).",
            ),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"]).default("M").describe(
                "Error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code.",
            ),
        )
        .param(
            Param::integer("size")
                .default(512)
                .min(64.0)
                .max(2048.0)
                .describe("PNG image edge in pixels (64-2048; ignored for svg)."),
        )
        .param(
            Param::string("dark_color")
                .default("#000000")
                .describe("Foreground (module) colour as #rgb or #rrggbb hex."),
        )
        .param(
            Param::string("light_color")
                .default("#ffffff")
                .describe("Background colour as #rgb or #rrggbb hex."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct QrGen;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qr-code-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a QR code (SVG or PNG)",
    skill(
        description = "Generate a QR code as an SVG or PNG image from text, a URL, Wi-Fi credentials, a contact card, an email, an SMS, a phone number, or a location. Set content to text (encode the data string), url (encode data as a link; https:// is prepended if no scheme), wifi (build a WIFI: join code from ssid/password/security/hidden — scanning offers to connect), contact (build a vCard from name/phone/email/organization/url), email (build a mailto: from email/subject/body), sms (build an SMSTO: from phone/body), phone (build a tel: from phone), or geo (build a geo: from latitude/longitude). Choose format svg (default, scalable) or png (raster; size sets the pixel edge, 64-2048). error_correction is L/M/Q/H (default M). dark_color/light_color recolour the code (#rgb or #rrggbb). Returns an image. Runs locally — the data never leaves the device.",
        parameters = schema_json()
    ),
)]
impl QrGen {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("qr-code-generator")?;
    let content = Content::parse(&args.content).map_err(SkillError::InvalidArgs)?;
    let format = Format::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let ecc = Ecc::parse(&args.error_correction).map_err(SkillError::InvalidArgs)?;
    let fields = Fields {
        data: args.data,
        ssid: args.ssid,
        password: args.password,
        security: args.security,
        hidden: args.hidden,
        name: args.name,
        phone: args.phone,
        email: args.email,
        organization: args.organization,
        url: args.url,
        subject: args.subject,
        body: args.body,
        latitude: args.latitude,
        longitude: args.longitude,
    };
    let g = generate(content, &fields, format, ecc, args.size, &args.dark_color, &args.light_color)
        .map_err(SkillError::InvalidArgs)?;
    let n = g.bytes.len();
    build_media_envelope(
        &g.bytes,
        g.format.mime(),
        format!("qr-code.{}", g.format.ext()),
        format!("QR code ({} image, {n} bytes) encoding: {}", g.format.ext().to_uppercase(), g.payload),
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
                    "content": { "type": "string", "enum": ["text", "url", "wifi", "contact", "email", "sms", "phone", "geo"], "default": "text", "description": "What to encode: text (any string), url (a link; a scheme is added if missing), wifi (join code from ssid/password/security), contact (a vCard from name/phone/email), email (a mailto: from email/subject/body), sms (an SMSTO: from phone/body), phone (a tel: from phone), or geo (a geo: from latitude/longitude)." },
                    "data": { "type": "string", "description": "The text or URL to encode (used when content is text or url)." },
                    "ssid": { "type": "string", "description": "Wi-Fi network name (content=wifi)." },
                    "password": { "type": "string", "description": "Wi-Fi password (content=wifi; required for WPA/WEP, omit for nopass)." },
                    "security": { "type": "string", "enum": ["WPA", "WEP", "nopass"], "default": "WPA", "description": "Wi-Fi security type (content=wifi): WPA (default, covers WPA/WPA2/WPA3), WEP, or nopass (open)." },
                    "hidden": { "type": "boolean", "default": false, "description": "Set true if the Wi-Fi SSID is hidden (content=wifi)." },
                    "name": { "type": "string", "description": "Contact full name (content=contact)." },
                    "phone": { "type": "string", "description": "Phone number — contact (content=contact), recipient (content=sms), or number to dial (content=phone)." },
                    "email": { "type": "string", "description": "Email address — contact (content=contact) or recipient (content=email)." },
                    "organization": { "type": "string", "description": "Contact organization (content=contact)." },
                    "url": { "type": "string", "description": "Contact website URL (content=contact)." },
                    "subject": { "type": "string", "description": "Email subject line (content=email)." },
                    "body": { "type": "string", "description": "Message body (content=email) or SMS text (content=sms)." },
                    "latitude": { "type": "string", "description": "Location latitude in decimal degrees (content=geo)." },
                    "longitude": { "type": "string", "description": "Location longitude in decimal degrees (content=geo)." },
                    "format": { "type": "string", "enum": ["svg", "png"], "default": "svg", "description": "Output image format: svg (scalable, default) or png (raster, sized by the size param)." },
                    "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "default": "M", "description": "Error-correction level: L (~7%), M (~15%, default), Q (~25%), or H (~30%). Higher survives more damage but makes a denser code." },
                    "size": { "type": "integer", "default": 512, "minimum": 64, "maximum": 2048, "description": "PNG image edge in pixels (64-2048; ignored for svg)." },
                    "dark_color": { "type": "string", "default": "#000000", "description": "Foreground (module) colour as #rgb or #rrggbb hex." },
                    "light_color": { "type": "string", "default": "#ffffff", "description": "Background colour as #rgb or #rrggbb hex." }
                },
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
