//! gizza-ai/resume-scaffolder — structured résumé inputs → print-ready HTML résumé.
//!
//! Thin chat-skill wrapper around `gizza-ai-resume-scaffolder-core`. Chat schema
//! single-sourced from `descriptor()`; handler delegates to `run_skill`. Pure.
//! Distinct from `resume-builder` (ATS Markdown): this emits a styled, self-
//! contained HTML document with a print stylesheet — ready to Print → Save-as-PDF.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_resume_scaffolder_core::{build, Font, Options, PageSize, Theme};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_accent")]
    accent: String,
    #[serde(default = "default_font")]
    font: String,
    #[serde(default = "default_page_size")]
    page_size: String,
}

fn default_theme() -> String {
    "modern".into()
}
fn default_accent() -> String {
    "#2563eb".into()
}
fn default_font() -> String {
    "sans".into()
}
fn default_page_size() -> String {
    "letter".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "A JSON object of résumé fields. Recognized: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and sections[{heading,items[]}] for extras like Projects/Certifications.",
            ),
        )
        .param(
            Param::enumv("theme", ["classic", "modern", "compact"])
                .default("modern")
                .describe(
                    "Layout style. 'classic' = serif, centered header, ink-ruled section titles. 'modern' (default) = sans, left header, an accent bar beside each section. 'compact' = tighter spacing to fit more on one page.",
                ),
        )
        .param(
            Param::string("accent")
                .default("#2563eb")
                .describe(
                    "Accent color for section titles and rules. A hex value like #2563eb or a plain CSS color name like navy. Default #2563eb (blue). Ignored visually by the 'classic' theme, which uses ink-black rules.",
                ),
        )
        .param(
            Param::enumv("font", ["sans", "serif"])
                .default("sans")
                .describe("Body font family. 'sans' (default) or 'serif'."),
        )
        .param(
            Param::enumv("page_size", ["letter", "a4"])
                .default("letter")
                .describe(
                    "Print page size for the embedded @page rule. 'letter' (US Letter, default) or 'a4'.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn render(a: Args) -> Result<String, String> {
    let opts = Options {
        theme: Theme::parse(&a.theme)?,
        accent: gizza_ai_resume_scaffolder_core::sanitize_accent(&a.accent)?,
        font: Font::parse(&a.font)?,
        page_size: PageSize::parse(&a.page_size)?,
    };
    build(&a.data, &opts)
}

#[cfg(target_arch = "wasm32")]
struct ResumeScaffolder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/resume-scaffolder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a clean, print-ready HTML résumé from structured details",
    skill(
        description = "Turn structured résumé details (a JSON object) into a clean, print-ready HTML résumé — a self-contained document with embedded styling and a print stylesheet, ready to open and Print → Save-as-PDF from the browser. Unlike an ATS/Markdown resume, this is a visually formatted document. `data` is a JSON object: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and optional sections[{heading,items[]}]. Style with theme (classic|modern|compact), accent (hex or CSS color name), font (sans|serif), and page_size (letter|a4). All résumé text is HTML-escaped.",
        parameters = schema_json()
    )
)]
impl ResumeScaffolder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "resume-scaffolder", |a: Args| {
            render(a).map_err(SkillError::InvalidArgs)
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
    fn render_applies_defaults_and_styles() {
        let html = render(Args {
            data: r#"{"name":"Ada"}"#.into(),
            theme: default_theme(),
            accent: default_accent(),
            font: default_font(),
            page_size: default_page_size(),
        })
        .unwrap();
        assert!(html.contains("theme-modern"));
        assert!(html.contains("--accent: #2563eb;"));
        assert!(html.contains("@page { size: letter;"));
    }

    #[test]
    fn render_rejects_bad_enum() {
        let err = render(Args {
            data: r#"{"name":"Ada"}"#.into(),
            theme: "sparkly".into(),
            accent: default_accent(),
            font: default_font(),
            page_size: default_page_size(),
        })
        .unwrap_err();
        assert!(err.contains("invalid theme"), "{err}");
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "A JSON object of résumé fields. Recognized: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and sections[{heading,items[]}] for extras like Projects/Certifications." },
                    "theme": { "type": "string", "enum": ["classic", "modern", "compact"], "default": "modern", "description": "Layout style. 'classic' = serif, centered header, ink-ruled section titles. 'modern' (default) = sans, left header, an accent bar beside each section. 'compact' = tighter spacing to fit more on one page." },
                    "accent": { "type": "string", "default": "#2563eb", "description": "Accent color for section titles and rules. A hex value like #2563eb or a plain CSS color name like navy. Default #2563eb (blue). Ignored visually by the 'classic' theme, which uses ink-black rules." },
                    "font": { "type": "string", "enum": ["sans", "serif"], "default": "sans", "description": "Body font family. 'sans' (default) or 'serif'." },
                    "page_size": { "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "Print page size for the embedded @page rule. 'letter' (US Letter, default) or 'a4'." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
