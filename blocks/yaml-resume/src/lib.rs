//! gizza-ai/yaml-resume — a JSON Resume document (YAML or JSON) → print-ready HTML.
//!
//! Thin chat-skill wrapper around `gizza-ai-yaml-resume-core`. Chat schema
//! single-sourced from `descriptor()`; handler delegates to `run_skill`. Pure.
//! Scoped to the **standard** JSON Resume v1.0.0 schema (`basics`/`work`/…),
//! which is what `resume-to-json` emits — the sibling `resume-scaffolder`
//! renders a different, bespoke flat shape and reads JSON only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_resume_core::{
    build, sanitize_accent, sanitize_font_size, sanitize_margin, sanitize_sections, DateFormat,
    Font, InputFormat, Options, PageSize, Theme,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_accent")]
    accent: String,
    #[serde(default = "default_font")]
    font: String,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_page_size")]
    page_size: String,
    #[serde(default = "default_margin")]
    margin: f64,
    #[serde(default = "default_date_format")]
    date_format: String,
    #[serde(default)]
    sections: String,
}

fn default_format() -> String {
    "auto".into()
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
fn default_font_size() -> f64 {
    10.5
}
fn default_page_size() -> String {
    "letter".into()
}
fn default_margin() -> f64 {
    0.5
}
fn default_date_format() -> String {
    "month-year".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "The résumé as a JSON Resume v1.0.0 document, written as YAML or JSON. Recognized top-level keys: basics (name required; label, email, phone, url, summary, location{address,postalCode,city,region,countryCode}, profiles[{network,username,url}]), work[{name,position,location,url,startDate,endDate,summary,highlights[]}], volunteer, education[{institution,area,studyType,score,courses[]}], projects[{name,description,highlights[],keywords[],roles[],entity,type}], skills[{name,level,keywords[]}], awards, certificates, publications, languages[{language,fluency}], interests, references[{name,reference}]. Unknown keys are ignored; an absent endDate reads as Present.",
            ),
        )
        .param(
            Param::enumv("format", ["auto", "yaml", "json"])
                .default("auto")
                .describe(
                    "How to parse 'data'. 'auto' (default) sniffs it — a document starting with { is tried as JSON first, anything else as YAML. Use 'yaml' or 'json' to force one and get that parser's error message.",
                ),
        )
        .param(
            Param::enumv("theme", ["classic", "modern", "compact", "ats"])
                .default("modern")
                .describe(
                    "Layout style. 'modern' (default) = sans, left header, an accent bar beside each section title. 'classic' = centered header with ink-ruled section titles. 'compact' = tighter spacing to fit more on one page. 'ats' = single column, no color and no rules — plain headings for résumé-parsing software.",
                ),
        )
        .param(
            Param::string("accent")
                .default("#2563eb")
                .describe(
                    "Accent color for section titles and rules. A hex value like #2563eb or a plain CSS color name like navy. Default #2563eb (blue). The 'classic' and 'ats' themes render headings in ink regardless.",
                ),
        )
        .param(
            Param::enumv("font", ["sans", "serif"])
                .default("sans")
                .describe("Body font family. 'sans' (default) or 'serif'."),
        )
        .param(
            Param::number("font_size")
                .default(10.5)
                .min(8.0)
                .max(14.0)
                .describe(
                    "Body type size in points (8-14). Default 10.5. Lower it to pull a long résumé onto one page.",
                ),
        )
        .param(
            Param::enumv("page_size", ["letter", "a4"])
                .default("letter")
                .describe(
                    "Print page size for the embedded @page rule. 'letter' (US Letter, default) or 'a4'.",
                ),
        )
        .param(
            Param::number("margin")
                .default(0.5)
                .min(0.25)
                .max(1.5)
                .describe(
                    "Print margin in inches on all four sides (0.25-1.5). Default 0.5. Also sets the on-screen page width.",
                ),
        )
        .param(
            Param::enumv("date_format", ["month-year", "year", "iso"])
                .default("month-year")
                .describe(
                    "How ISO-8601 dates are printed. 'month-year' (default) renders 2020-01-15 as Jan 2020; 'year' renders 2020; 'iso' prints the value exactly as written. Text that isn't a date (e.g. 'Summer 2019') always passes through unchanged.",
                ),
        )
        .param(
            Param::string("sections")
                .default("")
                .describe(
                    "Comma-separated section keys to render, in this order — any of summary, work, volunteer, education, projects, skills, awards, certificates, publications, languages, interests, references. Blank (default) renders every section that has content, in that canonical order.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn render(a: Args) -> Result<String, String> {
    let opts = Options {
        format: InputFormat::parse(&a.format)?,
        theme: Theme::parse(&a.theme)?,
        date_format: DateFormat::parse(&a.date_format)?,
        accent: sanitize_accent(&a.accent)?,
        font: Font::parse(&a.font)?,
        font_size: sanitize_font_size(a.font_size)?,
        page_size: PageSize::parse(&a.page_size)?,
        margin: sanitize_margin(a.margin)?,
        sections: sanitize_sections(&a.sections)?,
    };
    build(&a.data, &opts)
}

#[cfg(target_arch = "wasm32")]
struct YamlResume;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-resume",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a YAML or JSON Resume document as a print-ready HTML résumé",
    skill(
        description = "Render a JSON Resume v1.0.0 document — written as YAML or JSON — into a self-contained, print-ready HTML résumé: one file with embedded styling and a print stylesheet, ready to open and Print → Save-as-PDF from the browser. Covers the full standard schema (basics, work, volunteer, education, projects, skills, awards, certificates, publications, languages, interests, references) and pairs with resume-to-json, which produces documents in this format. `data` is the document; `format` picks the parser (auto|yaml|json). Style with theme (classic|modern|compact|ats — 'ats' is a plain single column for résumé parsers), accent (hex or CSS color name), font (sans|serif), font_size (8-14pt), page_size (letter|a4), margin (0.25-1.5in), date_format (month-year|year|iso), and sections to pick/reorder what appears. All résumé text is HTML-escaped and only http(s)/mailto URLs become links.",
        parameters = schema_json()
    )
)]
impl YamlResume {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-resume", |a: Args| {
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

    fn args(data: &str) -> Args {
        Args {
            data: data.into(),
            format: default_format(),
            theme: default_theme(),
            accent: default_accent(),
            font: default_font(),
            font_size: default_font_size(),
            page_size: default_page_size(),
            margin: default_margin(),
            date_format: default_date_format(),
            sections: String::new(),
        }
    }

    const YAML: &str = "basics:\n  name: Ada Lovelace\nwork:\n  - name: Analytical Co\n    position: Engineer\n    startDate: \"1843-01\"\n";

    #[test]
    fn render_applies_defaults_to_a_yaml_document() {
        let html = render(args(YAML)).unwrap();
        assert!(html.contains("<h1>Ada Lovelace</h1>"));
        assert!(html.contains("theme-modern"));
        assert!(html.contains("--accent: #2563eb;"));
        assert!(html.contains("--fs: 10.5pt;"));
        assert!(
            html.contains("@page { size: letter; margin: 0.5in; }"),
            "{html}"
        );
        assert!(html.contains("Jan 1843 – Present"), "{html}");
    }

    #[test]
    fn render_accepts_json_and_the_full_style_surface() {
        let mut a = args(r#"{"basics":{"name":"Grace Hopper"},"skills":[{"name":"Compilers"}]}"#);
        a.format = "json".into();
        a.theme = "ats".into();
        a.font = "serif".into();
        a.font_size = 12.0;
        a.page_size = "a4".into();
        a.margin = 0.75;
        a.date_format = "iso".into();
        a.sections = "skills".into();
        let html = render(a).unwrap();
        assert!(html.contains("theme-ats"));
        assert!(html.contains("Georgia"));
        assert!(
            html.contains("@page { size: A4; margin: 0.75in; }"),
            "{html}"
        );
        assert!(html.contains("<h2>Skills</h2>"));
    }

    #[test]
    fn render_rejects_bad_values_with_actionable_errors() {
        let mut a = args(YAML);
        a.theme = "sparkly".into();
        assert!(render(a).unwrap_err().contains("invalid theme"));

        let mut a = args(YAML);
        a.font_size = 40.0;
        assert!(render(a).unwrap_err().contains("font_size"));

        let mut a = args(YAML);
        a.sections = "work, hobbies".into();
        assert!(render(a).unwrap_err().contains("unknown section"));

        let mut a = args(YAML);
        a.format = "xml".into();
        assert!(render(a).unwrap_err().contains("invalid format"));

        assert!(render(args("basics:\n  label: Engineer\n"))
            .unwrap_err()
            .contains("basics.name"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The résumé as a JSON Resume v1.0.0 document, written as YAML or JSON. Recognized top-level keys: basics (name required; label, email, phone, url, summary, location{address,postalCode,city,region,countryCode}, profiles[{network,username,url}]), work[{name,position,location,url,startDate,endDate,summary,highlights[]}], volunteer, education[{institution,area,studyType,score,courses[]}], projects[{name,description,highlights[],keywords[],roles[],entity,type}], skills[{name,level,keywords[]}], awards, certificates, publications, languages[{language,fluency}], interests, references[{name,reference}]. Unknown keys are ignored; an absent endDate reads as Present." },
                    "format": { "type": "string", "enum": ["auto", "yaml", "json"], "default": "auto", "description": "How to parse 'data'. 'auto' (default) sniffs it — a document starting with { is tried as JSON first, anything else as YAML. Use 'yaml' or 'json' to force one and get that parser's error message." },
                    "theme": { "type": "string", "enum": ["classic", "modern", "compact", "ats"], "default": "modern", "description": "Layout style. 'modern' (default) = sans, left header, an accent bar beside each section title. 'classic' = centered header with ink-ruled section titles. 'compact' = tighter spacing to fit more on one page. 'ats' = single column, no color and no rules — plain headings for résumé-parsing software." },
                    "accent": { "type": "string", "default": "#2563eb", "description": "Accent color for section titles and rules. A hex value like #2563eb or a plain CSS color name like navy. Default #2563eb (blue). The 'classic' and 'ats' themes render headings in ink regardless." },
                    "font": { "type": "string", "enum": ["sans", "serif"], "default": "sans", "description": "Body font family. 'sans' (default) or 'serif'." },
                    "font_size": { "type": "number", "default": 10.5, "minimum": 8, "maximum": 14, "description": "Body type size in points (8-14). Default 10.5. Lower it to pull a long résumé onto one page." },
                    "page_size": { "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "Print page size for the embedded @page rule. 'letter' (US Letter, default) or 'a4'." },
                    "margin": { "type": "number", "default": 0.5, "minimum": 0.25, "maximum": 1.5, "description": "Print margin in inches on all four sides (0.25-1.5). Default 0.5. Also sets the on-screen page width." },
                    "date_format": { "type": "string", "enum": ["month-year", "year", "iso"], "default": "month-year", "description": "How ISO-8601 dates are printed. 'month-year' (default) renders 2020-01-15 as Jan 2020; 'year' renders 2020; 'iso' prints the value exactly as written. Text that isn't a date (e.g. 'Summer 2019') always passes through unchanged." },
                    "sections": { "type": "string", "default": "", "description": "Comma-separated section keys to render, in this order — any of summary, work, volunteer, education, projects, skills, awards, certificates, publications, languages, interests, references. Blank (default) renders every section that has content, in that canonical order." }
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
