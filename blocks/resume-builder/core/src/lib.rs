//! gizza-ai/resume-builder core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Renders a structured JSON resume
//! into clean, ATS-friendly Markdown (plain headings, no tables/columns/graphics
//! that ATS parsers choke on). Every field is optional; only what's present is
//! rendered.

use serde_json::Value;

fn s(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").trim().to_string()
}

fn str_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect())
        .unwrap_or_default()
}

/// Build an ATS-friendly Markdown resume from a JSON object. Recognized keys:
/// name, title, email, phone, location, links[], summary,
/// experience[{role,company,location,dates,bullets[]}],
/// education[{degree,school,location,dates,details}], skills[], plus any extra
/// `sections`[{heading, items[]}] for custom sections (Projects, Certifications…).
pub fn build(json: &str) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("input is empty".into());
    }
    let v: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    if !v.is_object() {
        return Err("expected a JSON object of resume fields".into());
    }

    let mut md = String::new();
    let name = s(&v, "name");
    if name.is_empty() {
        return Err("a 'name' field is required".into());
    }
    md.push_str(&format!("# {name}\n"));
    let title = s(&v, "title");
    if !title.is_empty() {
        md.push_str(&format!("\n{title}\n"));
    }
    // Contact line.
    let mut contact: Vec<String> = Vec::new();
    for k in ["email", "phone", "location"] {
        let val = s(&v, k);
        if !val.is_empty() {
            contact.push(val);
        }
    }
    contact.extend(str_list(&v, "links"));
    if !contact.is_empty() {
        md.push_str(&format!("\n{}\n", contact.join(" · ")));
    }

    let summary = s(&v, "summary");
    if !summary.is_empty() {
        md.push_str(&format!("\n## Summary\n\n{summary}\n"));
    }

    // Experience.
    if let Some(exp) = v.get("experience").and_then(Value::as_array) {
        if !exp.is_empty() {
            md.push_str("\n## Experience\n");
            for e in exp {
                let role = s(e, "role");
                let company = s(e, "company");
                let dates = s(e, "dates");
                let loc = s(e, "location");
                let head = [role.as_str(), company.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" — ");
                if !head.is_empty() {
                    md.push_str(&format!("\n### {head}\n"));
                }
                let meta = [dates.as_str(), loc.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
                if !meta.is_empty() {
                    md.push_str(&format!("*{meta}*\n"));
                }
                for b in str_list(e, "bullets") {
                    md.push_str(&format!("- {b}\n"));
                }
            }
        }
    }

    // Education.
    if let Some(ed) = v.get("education").and_then(Value::as_array) {
        if !ed.is_empty() {
            md.push_str("\n## Education\n");
            for e in ed {
                let degree = s(e, "degree");
                let school = s(e, "school");
                let dates = s(e, "dates");
                let head = [degree.as_str(), school.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" — ");
                if !head.is_empty() {
                    md.push_str(&format!("\n### {head}\n"));
                }
                let meta = [dates.as_str(), s(e, "location").as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
                if !meta.is_empty() {
                    md.push_str(&format!("*{meta}*\n"));
                }
                let details = s(e, "details");
                if !details.is_empty() {
                    md.push_str(&format!("{details}\n"));
                }
            }
        }
    }

    let skills = str_list(&v, "skills");
    if !skills.is_empty() {
        md.push_str(&format!("\n## Skills\n\n{}\n", skills.join(", ")));
    }

    // Custom sections.
    if let Some(sections) = v.get("sections").and_then(Value::as_array) {
        for sec in sections {
            let heading = s(sec, "heading");
            let items = str_list(sec, "items");
            if heading.is_empty() || items.is_empty() {
                continue;
            }
            md.push_str(&format!("\n## {heading}\n\n"));
            for it in items {
                md.push_str(&format!("- {it}\n"));
            }
        }
    }

    Ok(md.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESUME: &str = r#"{
        "name": "Ada Lovelace", "title": "Software Engineer",
        "email": "ada@example.com", "location": "London",
        "links": ["github.com/ada"],
        "summary": "Pioneering engineer.",
        "experience": [{"role":"Engineer","company":"Analytical Co","dates":"1843–1852","bullets":["Wrote the first algorithm","Designed loops"]}],
        "education": [{"degree":"Mathematics","school":"Home tutoring","dates":"1830s"}],
        "skills": ["Algorithms","Mathematics","Writing"]
    }"#;

    #[test]
    fn renders_full_resume() {
        let md = build(RESUME).unwrap();
        assert!(md.starts_with("# Ada Lovelace"));
        assert!(md.contains("ada@example.com · London · github.com/ada"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("### Engineer — Analytical Co"));
        assert!(md.contains("- Wrote the first algorithm"));
        assert!(md.contains("## Skills\n\nAlgorithms, Mathematics, Writing"));
    }

    #[test]
    fn minimal_resume_just_name() {
        let md = build(r#"{"name":"Bob"}"#).unwrap();
        assert_eq!(md, "# Bob");
    }

    #[test]
    fn custom_sections_render() {
        let md = build(r#"{"name":"X","sections":[{"heading":"Projects","items":["gizza","wafer"]}]}"#).unwrap();
        assert!(md.contains("## Projects"));
        assert!(md.contains("- gizza"));
    }

    #[test]
    fn missing_name_errors() {
        assert!(build(r#"{"email":"x@y.com"}"#).is_err());
    }

    #[test]
    fn bad_json_errors() {
        assert!(build("{not json").is_err());
        assert!(build("   ").is_err());
        assert!(build("[1,2]").is_err());
    }
}
