//! gizza-ai/job-posting-parser core — deterministic extraction from pasted job
//! descriptions into a compact JSON summary.
//!
//! This is intentionally heuristic and pure Rust: no model calls, no network, no
//! company-specific scraping. It looks for labelled fields, common header shapes,
//! salary patterns, location/remote hints, employment type, experience level, and
//! skill keywords, then reports the evidence it used.

use serde_json::json;

const MAX_INPUT_CHARS: usize = 80_000;
const SKILL_KEYWORDS: &[&str] = &[
    "Rust", "Python", "JavaScript", "TypeScript", "Go", "Java", "C++", "C#", "SQL", "PostgreSQL",
    "MySQL", "SQLite", "React", "Vue", "Angular", "Node.js", "Deno", "AWS", "Azure", "GCP",
    "Docker", "Kubernetes", "Terraform", "Linux", "Git", "CI/CD", "GraphQL", "REST", "gRPC",
    "Spark", "Kafka", "Pandas", "NumPy", "PyTorch", "TensorFlow", "LLM", "NLP", "Excel",
    "Tableau", "Power BI", "Salesforce", "Figma", "SEO", "HTML", "CSS", "Tailwind",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Markdown,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutputFormat::Json),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            other => Err(format!("output must be 'json' or 'markdown', got {other:?}")),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedJob {
    pub title: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub salary: Option<String>,
    pub employment_type: Option<String>,
    pub work_mode: Option<String>,
    pub experience_level: Option<String>,
    pub skills: Vec<String>,
    pub evidence: Vec<String>,
    pub warnings: Vec<String>,
}

/// Parse a pasted job posting and render the result as JSON or Markdown.
pub fn parse_job_posting(input: &str, output: &str, include_evidence: bool) -> Result<String, String> {
    let format = OutputFormat::parse(output)?;
    let text = input.trim();
    if text.is_empty() {
        return Err("posting is empty — paste a job description to parse".to_string());
    }
    if text.chars().count() > MAX_INPUT_CHARS {
        return Err(format!("posting is too large — maximum is {MAX_INPUT_CHARS} characters"));
    }

    let parsed = parse(text);
    if parsed.title.is_none() && parsed.company.is_none() && parsed.skills.is_empty() && parsed.salary.is_none() {
        return Err("posting does not look like a job ad: no title, company, salary, or skills were found".to_string());
    }

    Ok(match format {
        OutputFormat::Json => render_json(&parsed, include_evidence),
        OutputFormat::Markdown => render_markdown(&parsed, include_evidence),
    })
}

/// Back-compat/simple wrapper used by the scaffolded caller until descriptor args are wired.
pub fn run(input: &str) -> Result<String, String> {
    parse_job_posting(input, "json", true)
}

fn parse(text: &str) -> ParsedJob {
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.trim().trim_matches(['•', '-', '*', '–']).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    let lower = text.to_ascii_lowercase();
    let mut job = ParsedJob::default();

    job.title = labelled(&lines, &["title", "job title", "role", "position"])
        .or_else(|| infer_title(&lines));
    if let Some(v) = &job.title { job.evidence.push(format!("title: {v}")); }

    job.company = labelled(&lines, &["company", "employer", "organization", "organisation"])
        .or_else(|| infer_company(&lines));
    if let Some(v) = &job.company { job.evidence.push(format!("company: {v}")); }

    job.location = labelled(&lines, &["location", "office", "work location"])
        .or_else(|| find_location(&lines));
    if let Some(v) = &job.location { job.evidence.push(format!("location: {v}")); }

    job.salary = labelled(&lines, &["salary", "compensation", "pay", "range"])
        .or_else(|| find_salary(&lines));
    if let Some(v) = &job.salary { job.evidence.push(format!("salary: {v}")); }

    job.employment_type = find_employment_type(&lower);
    job.work_mode = find_work_mode(&lower);
    job.experience_level = find_experience_level(&lower);
    job.skills = find_skills(text);

    if job.salary.is_none() {
        job.warnings.push("salary/compensation was not found".to_string());
    }
    if job.location.is_none() && job.work_mode.is_none() {
        job.warnings.push("location or work mode was not found".to_string());
    }
    if job.skills.is_empty() {
        job.warnings.push("no known skill keywords were found".to_string());
    }
    job
}

fn labelled(lines: &[String], keys: &[&str]) -> Option<String> {
    for line in lines {
        let lower = line.to_ascii_lowercase();
        for key in keys {
            for sep in [":", "-"] {
                let prefix = format!("{key}{sep}");
                if lower.starts_with(&prefix) {
                    let v = line[prefix.len()..].trim();
                    if good_value(v) { return Some(clean(v)); }
                }
            }
        }
    }
    None
}

fn infer_title(lines: &[String]) -> Option<String> {
    lines.iter().take(4).find(|l| {
        let lc = l.to_ascii_lowercase();
        l.len() <= 90 && ["engineer", "developer", "manager", "analyst", "designer", "scientist", "specialist", "architect", "lead", "director", "intern"].iter().any(|w| lc.contains(w))
    }).map(|s| clean(s))
}

fn infer_company(lines: &[String]) -> Option<String> {
    for line in lines.iter().take(8) {
        let lc = line.to_ascii_lowercase();
        if let Some(rest) = lc.strip_prefix("at ") {
            if !rest.is_empty() { return Some(clean(&line[3..])); }
        }
        if lc.starts_with("about ") && line.len() <= 80 {
            return Some(clean(&line[6..]));
        }
    }
    None
}

fn find_location(lines: &[String]) -> Option<String> {
    let markers = ["remote", "hybrid", "onsite", "on-site", "new york", "san francisco", "london", "berlin", "toronto", "austin", "seattle"];
    lines.iter().take(12).find(|l| {
        let lc = l.to_ascii_lowercase();
        l.len() <= 100 && markers.iter().any(|m| lc.contains(m))
    }).map(|s| clean(s))
}

fn find_salary(lines: &[String]) -> Option<String> {
    for line in lines {
        let lc = line.to_ascii_lowercase();
        let has_money = line.contains('$') || line.contains('€') || line.contains('£') || lc.contains(" usd") || lc.contains(" eur") || lc.contains(" gbp");
        let has_pay_word = ["salary", "compensation", "base pay", "pay range", "hour", "year", "annum"].iter().any(|w| lc.contains(w));
        let has_digit = line.chars().any(|c| c.is_ascii_digit());
        if has_digit && (has_money || has_pay_word) {
            return Some(clean(line));
        }
    }
    None
}

fn find_employment_type(lower: &str) -> Option<String> {
    for (needle, label) in [
        ("full-time", "full-time"), ("full time", "full-time"), ("part-time", "part-time"),
        ("part time", "part-time"), ("contract", "contract"), ("internship", "internship"),
        ("temporary", "temporary"),
    ] {
        if lower.contains(needle) { return Some(label.to_string()); }
    }
    None
}

fn find_work_mode(lower: &str) -> Option<String> {
    if lower.contains("hybrid") { Some("hybrid".into()) }
    else if lower.contains("remote") || lower.contains("work from home") { Some("remote".into()) }
    else if lower.contains("onsite") || lower.contains("on-site") || lower.contains("in office") { Some("onsite".into()) }
    else { None }
}

fn find_experience_level(lower: &str) -> Option<String> {
    if lower.contains("intern") || lower.contains("entry level") || lower.contains("junior") { Some("entry/junior".into()) }
    else if lower.contains("senior") || lower.contains("staff") || lower.contains("principal") || lower.contains("lead") { Some("senior+".into()) }
    else if lower.contains("manager") || lower.contains("director") || lower.contains("head of") { Some("management".into()) }
    else { None }
}

fn find_skills(text: &str) -> Vec<String> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    for skill in SKILL_KEYWORDS {
        if contains_token(&lower, &skill.to_ascii_lowercase()) && !out.iter().any(|s| s == skill) {
            out.push((*skill).to_string());
        }
    }
    out
}

fn contains_token(haystack: &str, needle: &str) -> bool {
    if needle.contains('+') || needle.contains('#') || needle.contains('/') || needle.contains('.') {
        return haystack.contains(needle);
    }
    haystack.split(|c: char| !c.is_alphanumeric()).any(|w| w == needle)
}

fn good_value(s: &str) -> bool { !s.trim().is_empty() && s.trim().len() <= 180 }
fn clean(s: &str) -> String { s.trim().trim_matches(['.', ',']).trim().to_string() }

fn render_json(job: &ParsedJob, include_evidence: bool) -> String {
    let mut value = json!({
        "title": job.title,
        "company": job.company,
        "location": job.location,
        "salary": job.salary,
        "employment_type": job.employment_type,
        "work_mode": job.work_mode,
        "experience_level": job.experience_level,
        "skills": job.skills,
        "warnings": job.warnings,
    });
    if include_evidence {
        value["evidence"] = json!(job.evidence);
    }
    serde_json::to_string_pretty(&value).unwrap()
}

fn render_markdown(job: &ParsedJob, include_evidence: bool) -> String {
    let mut out = String::new();
    out.push_str("## Parsed job posting\n\n");
    for (label, value) in [
        ("Title", &job.title), ("Company", &job.company), ("Location", &job.location),
        ("Salary", &job.salary), ("Employment type", &job.employment_type),
        ("Work mode", &job.work_mode), ("Experience level", &job.experience_level),
    ] {
        out.push_str(&format!("- **{label}:** {}\n", value.as_deref().unwrap_or("not found")));
    }
    out.push_str(&format!("- **Skills:** {}\n", if job.skills.is_empty() { "none found".into() } else { job.skills.join(", ") }));
    if !job.warnings.is_empty() {
        out.push_str("\n### Warnings\n");
        for w in &job.warnings { out.push_str(&format!("- {w}\n")); }
    }
    if include_evidence && !job.evidence.is_empty() {
        out.push_str("\n### Evidence\n");
        for e in &job.evidence { out.push_str(&format!("- {e}\n")); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const POSTING: &str = "Senior Backend Engineer\nCompany: Acme Analytics\nLocation: Remote - US / Toronto\nCompensation: $150,000 - $185,000 USD\nFull-time\nWe need Rust, Python, PostgreSQL, Docker, Kubernetes, AWS, GraphQL and CI/CD experience.";

    #[test]
    fn extracts_core_fields_and_skills_as_json() {
        let out = parse_job_posting(POSTING, "json", true).unwrap();
        assert!(out.contains("Senior Backend Engineer"), "{out}");
        assert!(out.contains("Acme Analytics"), "{out}");
        assert!(out.contains("$150,000 - $185,000 USD"), "{out}");
        assert!(out.contains("remote"), "{out}");
        assert!(out.contains("senior+"), "{out}");
        assert!(out.contains("Rust") && out.contains("PostgreSQL") && out.contains("Kubernetes"), "{out}");
    }

    #[test]
    fn markdown_output_can_hide_evidence() {
        let out = parse_job_posting(POSTING, "markdown", false).unwrap();
        assert!(out.starts_with("## Parsed job posting"));
        assert!(out.contains("**Company:** Acme Analytics"));
        assert!(!out.contains("### Evidence"));
    }

    #[test]
    fn empty_or_low_signal_inputs_are_errors() {
        assert!(parse_job_posting("   ", "json", true).unwrap_err().contains("empty"));
        assert!(parse_job_posting("hello world", "json", true).unwrap_err().contains("does not look"));
    }

    #[test]
    fn invalid_output_is_an_error() {
        assert!(parse_job_posting(POSTING, "xml", true).unwrap_err().contains("output must"));
    }
}
