//! gizza-ai/ldif-to-csv — parse an LDAP LDIF export into CSV. Thin wrapper;
//! chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ldif_to_csv_core::{to_csv, MultiValue, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    ldif: String,
    #[serde(default)]
    columns: String,
    #[serde(default = "default_true")]
    include_dn: bool,
    #[serde(default)]
    multi_value: String,
    #[serde(default)]
    value_separator: String,
    #[serde(default = "default_max_indexed")]
    max_indexed: i64,
    #[serde(default = "default_true")]
    decode_base64: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    include_changetype: bool,
}
fn default_true() -> bool {
    true
}
fn default_max_indexed() -> i64 {
    10
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("ldif")
                .required()
                .describe("The LDIF text to convert (RFC 2849): 'dn:' lines, attributes, blank lines between entries. Folded continuation lines, '#' comments and a 'version: 1' header are handled."),
        )
        .param(
            Param::string("columns")
                .describe("Comma-separated attribute names to output, in that order (e.g. 'cn,mail,telephoneNumber'). Requested attributes missing from the file still get an empty column. Leave empty for every attribute, in first-seen order."),
        )
        .param(
            Param::boolean("include_dn")
                .default(true)
                .describe("Output the entry DN as a column (first, unless 'columns' places 'dn' elsewhere). Default true."),
        )
        .param(
            Param::enumv("multi_value", ["join", "indexed", "first", "last"])
                .default("join")
                .describe("How repeated attributes (objectClass, memberOf) map to cells: 'join' one cell joined by value_separator (default), 'indexed' one column each (attr, attr.2, attr.3…), 'first' or 'last' keeps a single value."),
        )
        .param(
            Param::string("value_separator")
                .default("|")
                .describe("Text placed between joined values when multi_value=join (also used for indexed overflow). Default '|'."),
        )
        .param(
            Param::integer("max_indexed")
                .default(10)
                .min(1.0)
                .max(50.0)
                .describe("Maximum columns per attribute when multi_value=indexed, 1-50 (default 10). Values beyond the cap are joined into the last column so nothing is lost."),
        )
        .param(
            Param::boolean("decode_base64")
                .default(true)
                .describe("Decode 'attr:: base64' values to text when the bytes are valid UTF-8 (binary values such as jpegPhoto stay base64). Default true; false keeps every '::' value encoded."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("CSV field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
        .param(
            Param::boolean("include_changetype")
                .describe("Add a 'changetype' column for LDIF change files (add/modify/delete). Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn to_options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        columns: a.columns.clone(),
        include_dn: a.include_dn,
        multi_value: MultiValue::parse(&a.multi_value)?,
        value_separator: if a.value_separator.is_empty() {
            "|".to_string()
        } else {
            a.value_separator.clone()
        },
        max_indexed: a.max_indexed,
        decode_base64: a.decode_base64,
        delimiter: if a.delimiter.is_empty() {
            ",".to_string()
        } else {
            a.delimiter.clone()
        },
        include_changetype: a.include_changetype,
    })
}

#[cfg(target_arch = "wasm32")]
struct LdifToCsv;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ldif-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an LDAP LDIF export into a CSV table",
    skill(
        description = "Convert an LDAP LDIF export (RFC 2849) into CSV: every `dn:` entry becomes one row and every attribute becomes a column. Handles folded continuation lines (a leading single space), `#` comments, a `version: 1` header, blank-line record separation, base64 `attr:: value` (decoded to text when it is valid UTF-8, kept encoded when binary), URL references `attr:< url` (the reference is kept as text — nothing is fetched), and change files, whose `changetype`/`add`/`replace`/`delete` directives are excluded from the data. Repeated attributes follow `multi_value`: join into one cell (default), spread across indexed columns (attr, attr.2, …), or keep the first/last. `columns` selects and orders attributes; `include_dn` controls the DN column; `delimiter` is a single char or comma/tab/semicolon/pipe. Attribute names are matched case-insensitively, column order is stable, missing values become empty cells, and output is RFC 4180 quoted. Fully local and deterministic — no AI model.",
        parameters = schema_json()
    ),
)]
impl LdifToCsv {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ldif-to-csv", |a: Args| {
            let opts = to_options(&a).map_err(SkillError::InvalidArgs)?;
            to_csv(&a.ldif, &opts).map_err(SkillError::InvalidArgs)
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
            r#"{
                "type": "object",
                "properties": {
                    "ldif": { "type": "string", "description": "The LDIF text to convert (RFC 2849): 'dn:' lines, attributes, blank lines between entries. Folded continuation lines, '#' comments and a 'version: 1' header are handled." },
                    "columns": { "type": "string", "description": "Comma-separated attribute names to output, in that order (e.g. 'cn,mail,telephoneNumber'). Requested attributes missing from the file still get an empty column. Leave empty for every attribute, in first-seen order." },
                    "include_dn": { "type": "boolean", "default": true, "description": "Output the entry DN as a column (first, unless 'columns' places 'dn' elsewhere). Default true." },
                    "multi_value": { "type": "string", "enum": ["join", "indexed", "first", "last"], "default": "join", "description": "How repeated attributes (objectClass, memberOf) map to cells: 'join' one cell joined by value_separator (default), 'indexed' one column each (attr, attr.2, attr.3…), 'first' or 'last' keeps a single value." },
                    "value_separator": { "type": "string", "default": "|", "description": "Text placed between joined values when multi_value=join (also used for indexed overflow). Default '|'." },
                    "max_indexed": { "type": "integer", "default": 10, "minimum": 1, "maximum": 50, "description": "Maximum columns per attribute when multi_value=indexed, 1-50 (default 10). Values beyond the cap are joined into the last column so nothing is lost." },
                    "decode_base64": { "type": "boolean", "default": true, "description": "Decode 'attr:: base64' values to text when the bytes are valid UTF-8 (binary values such as jpegPhoto stay base64). Default true; false keeps every '::' value encoded." },
                    "delimiter": { "type": "string", "default": ",", "description": "CSV field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "include_changetype": { "type": "boolean", "description": "Add a 'changetype' column for LDIF change files (add/modify/delete). Default false." }
                },
                "required": ["ldif"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn options_reject_an_unknown_multi_value_policy() {
        let a = Args {
            ldif: "dn: cn=x".into(),
            columns: String::new(),
            include_dn: true,
            multi_value: "sideways".into(),
            value_separator: String::new(),
            max_indexed: 10,
            decode_base64: true,
            delimiter: String::new(),
            include_changetype: false,
        };
        let err = to_options(&a).unwrap_err();
        assert!(err.contains("join/indexed/first/last"), "{err}");
    }

    #[test]
    fn empty_option_strings_fall_back_to_defaults() {
        let a = Args {
            ldif: "dn: cn=x".into(),
            columns: String::new(),
            include_dn: true,
            multi_value: String::new(),
            value_separator: String::new(),
            max_indexed: 10,
            decode_base64: true,
            delimiter: String::new(),
            include_changetype: false,
        };
        let o = to_options(&a).unwrap();
        assert_eq!(o.multi_value, MultiValue::Join);
        assert_eq!(o.value_separator, "|");
        assert_eq!(o.delimiter, ",");
    }
}
