//! gizza-ai/vcard-qr — build a vCard (.vcf) from contact details and render it
//! as a scannable QR code SVG that saves the contact when scanned.
//!
//! Pure-Rust (`qrcode`), so it runs on ALL backends including the chat Service
//! Worker. The SVG is wrapped as an `image/svg+xml` data-URL envelope for
//! chat/CLI; the standalone page renders the same SVG inline. The exact vCard
//! source travels with it — in the SVG `<desc>` and in the envelope summary — so
//! it can be pasted straight into a `.vcf` file.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_vcard_qr_core::{Options, MAX_SIZE, MIN_SIZE};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
struct Args {
    first_name: String,
    last_name: String,
    organization: String,
    job_title: String,
    mobile: String,
    phone: String,
    email: String,
    website: String,
    street: String,
    city: String,
    region: String,
    postal_code: String,
    country: String,
    note: String,
    birthday: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default = "default_ecc")]
    error_correction: String,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default = "default_foreground")]
    foreground: String,
    #[serde(default = "default_background")]
    background: String,
    #[serde(default = "default_show_details")]
    show_details: bool,
}

fn default_version() -> String {
    "3.0".to_string()
}
fn default_ecc() -> String {
    "M".to_string()
}
fn default_size() -> u32 {
    512
}
fn default_foreground() -> String {
    "#000000".to_string()
}
fn default_background() -> String {
    "#ffffff".to_string()
}
fn default_show_details() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("first_name")
                .describe(
                    "The contact's given name, e.g. Ada. Goes into the vCard N and FN properties. At least one of first_name, last_name or organization is required — a vCard with no name cannot be saved by a phone.",
                ),
        )
        .param(
            Param::string("last_name")
                .describe(
                    "The contact's family name, e.g. Lovelace. Combined with first_name into the display name (FN) that phones show in the contact list.",
                ),
        )
        .param(
            Param::string("organization")
                .describe(
                    "Company or organization name (vCard ORG), e.g. Analytical Engines. Used as the display name when no personal name is given, which is what you want for a company card.",
                ),
        )
        .param(
            Param::string("job_title")
                .describe("Job title or role (vCard TITLE), e.g. Chief Analyst."),
        )
        .param(
            Param::string("mobile")
                .describe(
                    "Mobile/cell number, saved as TEL;TYPE=CELL. Write it the way it should be dialled, ideally in international form, e.g. +44 7700 900123. Spaces and punctuation are kept verbatim.",
                ),
        )
        .param(
            Param::string("phone")
                .describe(
                    "Landline or office number, saved as TEL;TYPE=WORK,VOICE, e.g. +1 202 555 0142. Use mobile for the cell number so phones label the two differently.",
                ),
        )
        .param(
            Param::string("email")
                .describe(
                    "Email address (vCard EMAIL), e.g. ada@example.com. Checked for a single @ and a dotted domain, so a typo is rejected instead of being encoded into a code nobody can fix later.",
                ),
        )
        .param(
            Param::string("website")
                .describe(
                    "Website or profile URL (vCard URL), e.g. example.com/ada. A bare host gets https:// added automatically so scanners can open it.",
                ),
        )
        .param(
            Param::string("street")
                .describe("Street address line of the postal address (vCard ADR street component), e.g. 12 Baker Street."),
        )
        .param(Param::string("city").describe("City or locality of the postal address, e.g. London."))
        .param(
            Param::string("region")
                .describe("State, province or region of the postal address, e.g. California or Greater London."),
        )
        .param(
            Param::string("postal_code")
                .describe("Postal or ZIP code of the postal address, e.g. NW1 6XE or 94103."),
        )
        .param(
            Param::string("country")
                .describe("Country of the postal address, e.g. United Kingdom. The five address fields are combined into one ADR;TYPE=WORK property; fill only the ones you have."),
        )
        .param(
            Param::string("note")
                .describe(
                    "Free-text note stored with the contact (vCard NOTE), e.g. Met at the 2026 expo. Long notes make the QR denser — keep it short if the code will be printed small.",
                ),
        )
        .param(
            Param::string("birthday")
                .describe(
                    "Birthday as an ISO date, YYYY-MM-DD (YYYYMMDD is also accepted), e.g. 1987-04-23. Saved as BDAY; impossible dates like 2023-02-29 are rejected.",
                ),
        )
        .param(
            Param::enumv("version", ["3.0", "4.0"])
                .default("3.0")
                .describe(
                    "vCard version to emit. 3.0 (default) is what phone cameras and address books recognise most reliably. 4.0 (RFC 6350) is the newer standard and uses lower-case TYPE values; pick it when the card is consumed by modern software rather than scanned.",
                ),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"])
                .default("M")
                .describe(
                    "QR error-correction level. L is the least dense and fits the most contact detail; M is the balanced default; Q and H survive smudges, folds and printing but make the code denser. Use Q or H for anything printed, L if a long contact will not fit.",
                ),
        )
        .param(
            Param::integer("size")
                .default(512)
                .min(MIN_SIZE as f64)
                .max(MAX_SIZE as f64)
                .describe(
                    "Width of the rendered SVG in pixels (128-2048, default 512). The output is vector, so this only sets the default display size — it stays sharp at any print scale.",
                ),
        )
        .param(
            Param::string("foreground")
                .default("#000000")
                .describe(
                    "Colour of the dark QR modules and the caption text, as a hex value (#rgb, #rrggbb, #rrggbbaa) or a CSS colour name. Keep strong contrast against the background or scanners will fail. Default #000000.",
                ),
        )
        .param(
            Param::string("background")
                .default("#ffffff")
                .describe(
                    "Colour behind the QR code, as a hex value or a CSS colour name; 'transparent' is accepted for overlaying on artwork. Default #ffffff.",
                ),
        )
        .param(
            Param::boolean("show_details")
                .default(true)
                .describe(
                    "Print the readable contact details (name, title and company, phone numbers, email, website) as monospace text under the code — handy for badges and business cards. Default true. Set false for a bare square code.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VcardQr;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vcard-qr",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a vCard from contact details and render it as a scannable QR code",
    skill(
        description = "Build a vCard (.vcf) contact card from individual fields and render it as a scannable QR code SVG — scanning it offers to save the contact on iOS and Android. Takes first_name, last_name, organization, job_title, mobile, phone, email, website, a postal address (street, city, region, postal_code, country), note and birthday, and emits an RFC 6350 / RFC 2426 vCard with correct escaping and 75-octet line folding. Choose vCard version 3.0 (default, best scanner compatibility) or 4.0, tune error_correction (L/M/Q/H), size, and foreground/background colours, and set show_details to print the readable contact block under the code for badges. Inputs are validated — a malformed email, an impossible birthday, a bad colour or a contact too long to encode fails with an explanation instead of producing an unscannable code. Returns an SVG image whose <desc> carries the exact vCard source. Runs locally — contact details never leave the device, and the code is static, so there is no tracking or expiry.",
        parameters = schema_json()
    ),
)]
impl VcardQr {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("vcard-qr")?;
    let opts = Options {
        first_name: &args.first_name,
        last_name: &args.last_name,
        organization: &args.organization,
        job_title: &args.job_title,
        mobile: &args.mobile,
        phone: &args.phone,
        email: &args.email,
        website: &args.website,
        street: &args.street,
        city: &args.city,
        region: &args.region,
        postal_code: &args.postal_code,
        country: &args.country,
        note: &args.note,
        birthday: &args.birthday,
        version: &args.version,
        error_correction: &args.error_correction,
        size: args.size,
        foreground: &args.foreground,
        background: &args.background,
        show_details: args.show_details,
    };
    let (vcard, svg) = gizza_ai_vcard_qr_core::render(&opts).map_err(SkillError::InvalidArgs)?;
    let stem = file_stem(&args);
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        format!("vcard-qr-{stem}.svg"),
        format!("Contact QR code. vCard source:\n{vcard}"),
        MAX_OUTPUT_BYTES,
    )
}

/// A filesystem-safe stem from the contact's name, so downloads are identifiable.
#[cfg(target_arch = "wasm32")]
fn file_stem(args: &Args) -> String {
    let raw = [&args.first_name, &args.last_name, &args.organization]
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let stem: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches('-').to_string();
    if stem.is_empty() {
        "contact".to_string()
    } else {
        stem.chars().take(48).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        // Drift guard: the authored schema below is what chat + the CLI consume.
        // If descriptor() changes, update this literal deliberately.
        let expected = serde_json::json!({
            "type": "object",
            "properties": {
                "first_name": {
                    "type": "string",
                    "description": "The contact's given name, e.g. Ada. Goes into the vCard N and FN properties. At least one of first_name, last_name or organization is required — a vCard with no name cannot be saved by a phone."
                },
                "last_name": {
                    "type": "string",
                    "description": "The contact's family name, e.g. Lovelace. Combined with first_name into the display name (FN) that phones show in the contact list."
                },
                "organization": {
                    "type": "string",
                    "description": "Company or organization name (vCard ORG), e.g. Analytical Engines. Used as the display name when no personal name is given, which is what you want for a company card."
                },
                "job_title": {
                    "type": "string",
                    "description": "Job title or role (vCard TITLE), e.g. Chief Analyst."
                },
                "mobile": {
                    "type": "string",
                    "description": "Mobile/cell number, saved as TEL;TYPE=CELL. Write it the way it should be dialled, ideally in international form, e.g. +44 7700 900123. Spaces and punctuation are kept verbatim."
                },
                "phone": {
                    "type": "string",
                    "description": "Landline or office number, saved as TEL;TYPE=WORK,VOICE, e.g. +1 202 555 0142. Use mobile for the cell number so phones label the two differently."
                },
                "email": {
                    "type": "string",
                    "description": "Email address (vCard EMAIL), e.g. ada@example.com. Checked for a single @ and a dotted domain, so a typo is rejected instead of being encoded into a code nobody can fix later."
                },
                "website": {
                    "type": "string",
                    "description": "Website or profile URL (vCard URL), e.g. example.com/ada. A bare host gets https:// added automatically so scanners can open it."
                },
                "street": {
                    "type": "string",
                    "description": "Street address line of the postal address (vCard ADR street component), e.g. 12 Baker Street."
                },
                "city": {
                    "type": "string",
                    "description": "City or locality of the postal address, e.g. London."
                },
                "region": {
                    "type": "string",
                    "description": "State, province or region of the postal address, e.g. California or Greater London."
                },
                "postal_code": {
                    "type": "string",
                    "description": "Postal or ZIP code of the postal address, e.g. NW1 6XE or 94103."
                },
                "country": {
                    "type": "string",
                    "description": "Country of the postal address, e.g. United Kingdom. The five address fields are combined into one ADR;TYPE=WORK property; fill only the ones you have."
                },
                "note": {
                    "type": "string",
                    "description": "Free-text note stored with the contact (vCard NOTE), e.g. Met at the 2026 expo. Long notes make the QR denser — keep it short if the code will be printed small."
                },
                "birthday": {
                    "type": "string",
                    "description": "Birthday as an ISO date, YYYY-MM-DD (YYYYMMDD is also accepted), e.g. 1987-04-23. Saved as BDAY; impossible dates like 2023-02-29 are rejected."
                },
                "version": {
                    "type": "string",
                    "enum": ["3.0", "4.0"],
                    "default": "3.0",
                    "description": "vCard version to emit. 3.0 (default) is what phone cameras and address books recognise most reliably. 4.0 (RFC 6350) is the newer standard and uses lower-case TYPE values; pick it when the card is consumed by modern software rather than scanned."
                },
                "error_correction": {
                    "type": "string",
                    "enum": ["L", "M", "Q", "H"],
                    "default": "M",
                    "description": "QR error-correction level. L is the least dense and fits the most contact detail; M is the balanced default; Q and H survive smudges, folds and printing but make the code denser. Use Q or H for anything printed, L if a long contact will not fit."
                },
                "size": {
                    "type": "integer",
                    "default": 512,
                    "minimum": 128,
                    "maximum": 2048,
                    "description": "Width of the rendered SVG in pixels (128-2048, default 512). The output is vector, so this only sets the default display size — it stays sharp at any print scale."
                },
                "foreground": {
                    "type": "string",
                    "default": "#000000",
                    "description": "Colour of the dark QR modules and the caption text, as a hex value (#rgb, #rrggbb, #rrggbbaa) or a CSS colour name. Keep strong contrast against the background or scanners will fail. Default #000000."
                },
                "background": {
                    "type": "string",
                    "default": "#ffffff",
                    "description": "Colour behind the QR code, as a hex value or a CSS colour name; 'transparent' is accepted for overlaying on artwork. Default #ffffff."
                },
                "show_details": {
                    "type": "boolean",
                    "default": true,
                    "description": "Print the readable contact details (name, title and company, phone numbers, email, website) as monospace text under the code — handy for badges and business cards. Default true. Set false for a bare square code."
                }
            },
            "additionalProperties": false
        });
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(actual, expected);
    }
}
