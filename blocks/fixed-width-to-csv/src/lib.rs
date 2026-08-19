//! gizza-ai/fixed-width-to-csv — split fixed-width / column-positional text
//! records into CSV. Thin wrapper; the chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fixed_width_to_csv_core::{convert, HeaderMode, Options, Quote};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    spec: String,
    #[serde(default = "default_header")]
    header: String,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_quote")]
    quote: String,
    #[serde(default = "default_newline")]
    newline: String,
    #[serde(default)]
    skip_lines: u32,
    #[serde(default)]
    comment: String,
    #[serde(default = "default_true")]
    skip_blank: bool,
    #[serde(default)]
    bom: bool,
}
fn default_true() -> bool {
    true
}
fn default_header() -> String {
    "first-row".to_string()
}
fn default_quote() -> String {
    "minimal".to_string()
}
fn default_newline() -> String {
    "lf".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The fixed-width text to convert: one record per line, columns aligned by character position (e.g. a mainframe extract, a COBOL copybook dump, or `ls -l`-style report output). Paste it as-is; trailing carriage returns are ignored."),
        )
        .param(
            Param::string("spec")
                .default("")
                .describe("Column layout. Leave EMPTY (default) to auto-detect the boundaries from the whitespace that lines up on every line. Otherwise give comma-separated columns: a width ('10'), a 1-based inclusive range ('1-10'), '*' for the rest of the line, or any of those with a name ('name:10', 'dob:11-18', 'notes:*'). Bare widths run consecutively from position 1, so '10,4,*' means characters 1-10, 11-14, 15-end. A spec that contains '|' uses the position,length[,name] form instead: '1,10,name|11,4,age|15,*,city' (length '*' or 0 = rest of the line). Positions count characters, not bytes."),
        )
        .param(
            Param::enumv("header", ["first-row", "names", "generate", "none"])
                .default("first-row")
                .describe("Where the CSV header row comes from: 'first-row' (default) treats the first line of input as the column titles; 'names' uses the names given in the column spec; 'generate' emits col1,col2,…; 'none' writes data rows only."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Strip leading and trailing whitespace from every extracted field (default true). Turn it off to keep each field's original padding byte-for-byte."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Output field separator: a single character, or one of 'comma' (default), 'tab' (gives TSV), 'semicolon', 'pipe', 'space', 'colon'."),
        )
        .param(
            Param::enumv("quote", ["minimal", "all", "never"])
                .default("minimal")
                .describe("CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a double quote, or a newline (RFC 4180); 'all' wraps every field in double quotes; 'never' never quotes — smaller output, but a field containing the delimiter will re-import as two fields."),
        )
        .param(
            Param::enumv("newline", ["lf", "crlf"])
                .default("lf")
                .describe("Output line ending between rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel)."),
        )
        .param(
            Param::integer("skip_lines")
                .default(0)
                .min(0.0)
                .max(10000.0)
                .describe("Drop this many lines from the top of the input before converting — use it to skip a report banner or page title. Default 0."),
        )
        .param(
            Param::string("comment")
                .default("")
                .describe("Ignore lines whose first non-space characters are this prefix, e.g. '#' or '//'. Empty (default) keeps every line."),
        )
        .param(
            Param::boolean("skip_blank")
                .default(true)
                .describe("Ignore blank and whitespace-only lines (default true). Turn it off to emit an all-empty CSV row wherever the input has a blank line."),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a UTF-8 byte-order mark (BOM) so Excel opens the CSV as UTF-8. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build(a: &Args) -> Result<Options, SkillError> {
    let quote = Quote::parse(&a.quote).map_err(SkillError::InvalidArgs)?;
    let header = HeaderMode::parse(&a.header).map_err(SkillError::InvalidArgs)?;
    let crlf = match a.newline.trim().to_ascii_lowercase().as_str() {
        "" | "lf" | "\n" => false,
        "crlf" | "\r\n" => true,
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "unknown newline '{other}' (use lf or crlf)"
            )))
        }
    };
    Ok(Options {
        spec: a.spec.clone(),
        min_gap: 1,
        header,
        trim: a.trim,
        delimiter: if a.delimiter.is_empty() {
            ",".to_string()
        } else {
            a.delimiter.clone()
        },
        quote,
        crlf,
        bom: a.bom,
        skip_lines: a.skip_lines as usize,
        comment: a.comment.clone(),
        skip_blank: a.skip_blank,
    })
}

#[cfg(target_arch = "wasm32")]
struct FixedWidthToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fixed-width-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split fixed-width column text into CSV, with auto-detected or explicit column positions",
    skill(
        description = "Convert fixed-width / column-positional text records into CSV. With no spec it auto-detects the column boundaries: any character position where EVERY line has a space (or has already ended) is a boundary, and the runs between them become columns. Give `spec` to pin the layout instead — comma-separated widths ('10,4,*'), 1-based inclusive ranges ('1-10,11-14'), named columns ('name:10,age:4,city:*'), or the pipe form '1,10,name|11,4,age|15,*,city'. Positions count characters, not bytes. header=first-row (default) uses the first line as titles; 'names' takes them from the spec; 'generate' emits col1,col2,…; 'none' writes data only. trim=true (default) strips field padding. delimiter sets the output separator (single char or comma/tab/semicolon/pipe/space/colon; 'tab' gives TSV). quote=minimal (default)/all/never; newline=lf (default)/crlf; bom=true prepends a UTF-8 BOM for Excel. skip_lines drops leading banner lines, `comment` ignores lines starting with a prefix, skip_blank=true (default) drops blank lines. Short lines yield empty fields; up to 50000 lines and 512 columns per run. Runs locally.",
        parameters = schema_json()
    ),
)]
impl FixedWidthToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fixed-width-to-csv", |a: Args| {
            let opts = build(&a)?;
            convert(&a.text, &opts).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    const AUTHORED: &str = r#"{
        "type": "object",
        "properties": {
            "text": { "type": "string", "description": "The fixed-width text to convert: one record per line, columns aligned by character position (e.g. a mainframe extract, a COBOL copybook dump, or `ls -l`-style report output). Paste it as-is; trailing carriage returns are ignored." },
            "spec": { "type": "string", "default": "", "description": "Column layout. Leave EMPTY (default) to auto-detect the boundaries from the whitespace that lines up on every line. Otherwise give comma-separated columns: a width ('10'), a 1-based inclusive range ('1-10'), '*' for the rest of the line, or any of those with a name ('name:10', 'dob:11-18', 'notes:*'). Bare widths run consecutively from position 1, so '10,4,*' means characters 1-10, 11-14, 15-end. A spec that contains '|' uses the position,length[,name] form instead: '1,10,name|11,4,age|15,*,city' (length '*' or 0 = rest of the line). Positions count characters, not bytes." },
            "header": { "type": "string", "enum": ["first-row", "names", "generate", "none"], "default": "first-row", "description": "Where the CSV header row comes from: 'first-row' (default) treats the first line of input as the column titles; 'names' uses the names given in the column spec; 'generate' emits col1,col2,…; 'none' writes data rows only." },
            "trim": { "type": "boolean", "default": true, "description": "Strip leading and trailing whitespace from every extracted field (default true). Turn it off to keep each field's original padding byte-for-byte." },
            "delimiter": { "type": "string", "default": ",", "description": "Output field separator: a single character, or one of 'comma' (default), 'tab' (gives TSV), 'semicolon', 'pipe', 'space', 'colon'." },
            "quote": { "type": "string", "enum": ["minimal", "all", "never"], "default": "minimal", "description": "CSV quoting: 'minimal' (default) quotes a field only when it contains the delimiter, a double quote, or a newline (RFC 4180); 'all' wraps every field in double quotes; 'never' never quotes — smaller output, but a field containing the delimiter will re-import as two fields." },
            "newline": { "type": "string", "enum": ["lf", "crlf"], "default": "lf", "description": "Output line ending between rows: 'lf' (default, `\\n`, Unix/macOS) or 'crlf' (`\\r\\n`, Windows/Excel)." },
            "skip_lines": { "type": "integer", "default": 0, "minimum": 0, "maximum": 10000, "description": "Drop this many lines from the top of the input before converting — use it to skip a report banner or page title. Default 0." },
            "comment": { "type": "string", "default": "", "description": "Ignore lines whose first non-space characters are this prefix, e.g. '#' or '//'. Empty (default) keeps every line." },
            "skip_blank": { "type": "boolean", "default": true, "description": "Ignore blank and whitespace-only lines (default true). Turn it off to emit an all-empty CSV row wherever the input has a blank line." },
            "bom": { "type": "boolean", "default": false, "description": "Prepend a UTF-8 byte-order mark (BOM) so Excel opens the CSV as UTF-8. Default false." }
        },
        "required": ["text"],
        "additionalProperties": false
    }"#;
}
