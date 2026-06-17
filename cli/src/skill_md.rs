//! Generate a `SKILL.md` document that teaches an LLM agent how to drive the
//! `gizza` CLI.
use crate::runtime::ToolMeta;

/// Render a complete `SKILL.md` document from the given tool metadata.
///
/// The output is deterministic: tools appear sorted by `short` name and the
/// JSON schema blocks use [`serde_json::to_string_pretty`] (alphabetic key
/// order is preserved from whatever order the schema was built with).
///
/// # Structure
/// - YAML frontmatter (`---`) with `name` and `description`.
/// - `## Usage` — command contract, all flags, exit codes, and a concrete
///   example.
/// - `## Tools` — one `### <short>` subsection per tool with description and
///   fenced `json` parameters block.
pub fn render_skill_md(tools: &[ToolMeta]) -> String {
    let mut out = String::new();

    // --- YAML frontmatter ---
    out.push_str("---\n");
    out.push_str("name: gizza-tools\n");
    out.push_str("description: Run gizza's local compute tools (calculator, clock, image/video, fetch, …) via the `gizza` CLI binary installed in the repo.\n");
    out.push_str("---\n\n");

    // --- Usage ---
    out.push_str("## Usage\n\n");
    out.push_str("All tools are invoked through the `gizza` binary.  Run from the `gizza-ai` repo root.\n\n");
    out.push_str("### Invoke a tool\n\n");
    out.push_str("```sh\n");
    out.push_str("# Positional: bare values fill required scalar fields left-to-right\n");
    out.push_str("gizza tool <name> <arg>\n\n");
    out.push_str("# Key=value pairs (mixed freely with positionals)\n");
    out.push_str("gizza tool <name> key=value ...\n\n");
    out.push_str("# Full JSON body\n");
    out.push_str("gizza tool <name> --json '{\"key\":\"value\"}'\n\n");
    out.push_str("# Print the complete JSON response envelope\n");
    out.push_str("gizza tool <name> <args> --json-out\n\n");
    out.push_str("# Write binary output to a file (image, video, …)\n");
    out.push_str("gizza tool <name> <args> --out result.png\n");
    out.push_str("```\n\n");
    out.push_str("### Discover tools\n\n");
    out.push_str("```sh\n");
    out.push_str("gizza list                 # table of short-name + description\n");
    out.push_str("gizza list --json-out       # JSON array\n");
    out.push_str("gizza describe <name>       # full schema for one tool\n");
    out.push_str("gizza describe <name> --json-out\n");
    out.push_str("```\n\n");
    out.push_str("### Generate / check this file\n\n");
    out.push_str("```sh\n");
    out.push_str("gizza gen-skill             # (re)write SKILL.md in the current directory\n");
    out.push_str("gizza gen-skill --check     # exit 0 if up-to-date, exit 1 if stale\n");
    out.push_str("```\n\n");
    out.push_str("### Exit codes\n\n");
    out.push_str("| Code | Meaning |\n");
    out.push_str("|------|---------|\n");
    out.push_str("| 0    | Success |\n");
    out.push_str("| 1    | Tool error (invalid input, compute failure) |\n");
    out.push_str("| 2    | Usage error (unknown tool, missing required arg) |\n");
    out.push_str("| 3    | Unsupported in CLI (e.g. `imagine` requires a browser GPU; use gizza.ai) |\n");
    out.push_str("\n");
    out.push_str("### Example\n\n");
    out.push_str("```sh\n");
    out.push_str("$ gizza tool calculator \"2*2\"\n");
    out.push_str("4\n");
    out.push_str("```\n\n");

    // --- Tools ---
    out.push_str("## Tools\n\n");

    let mut sorted: Vec<&ToolMeta> = tools.iter().collect();
    sorted.sort_by(|a, b| a.short.cmp(&b.short));

    for tool in sorted {
        out.push_str(&format!("### {}\n\n", tool.short));
        out.push_str(&format!("{}\n\n", tool.description));
        out.push_str("```json\n");
        out.push_str(&serde_json::to_string_pretty(&tool.parameters).unwrap_or_default());
        out.push('\n');
        out.push_str("```\n\n");
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_calc_meta() -> ToolMeta {
        ToolMeta {
            name: "gizza-ai/calculator".to_string(),
            short: "calculator".to_string(),
            description: "Evaluate a mathematical expression and return the result.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expr": {
                        "type": "string",
                        "description": "The expression to evaluate, e.g. \"2+2\""
                    }
                },
                "required": ["expr"]
            }),
        }
    }

    #[test]
    fn contains_tool_heading() {
        let tools = vec![make_calc_meta()];
        let md = render_skill_md(&tools);
        assert!(
            md.contains("### calculator"),
            "should have a ### calculator heading; got:\n{md}"
        );
    }

    #[test]
    fn contains_expr_property() {
        let tools = vec![make_calc_meta()];
        let md = render_skill_md(&tools);
        assert!(
            md.contains("\"expr\""),
            "should include expr property in schema; got:\n{md}"
        );
    }

    #[test]
    fn contains_gizza_tool_calculator_example() {
        let tools = vec![make_calc_meta()];
        let md = render_skill_md(&tools);
        assert!(
            md.contains("gizza tool calculator"),
            "should have gizza tool calculator in the usage example; got:\n{md}"
        );
    }

    #[test]
    fn frontmatter_present() {
        let tools = vec![make_calc_meta()];
        let md = render_skill_md(&tools);
        assert!(md.starts_with("---\n"), "must start with YAML frontmatter");
        assert!(
            md.contains("name: gizza-tools"),
            "frontmatter must have name: gizza-tools"
        );
    }

    #[test]
    fn tools_sorted_alphabetically() {
        let clock = ToolMeta {
            name: "gizza-ai/clock".to_string(),
            short: "clock".to_string(),
            description: "Return current UTC time.".to_string(),
            parameters: json!({"type": "object", "properties": {}, "required": []}),
        };
        let calc = make_calc_meta();
        // Pass in reverse order; output should be sorted.
        let tools = vec![clock, calc];
        let md = render_skill_md(&tools);
        let calc_pos = md.find("### calculator").expect("calculator heading");
        let clock_pos = md.find("### clock").expect("clock heading");
        assert!(
            calc_pos < clock_pos,
            "calculator should sort before clock alphabetically"
        );
    }
}
