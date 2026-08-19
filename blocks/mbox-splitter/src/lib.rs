//! gizza-ai/mbox-splitter — split a multi-message mbox archive into the
//! individual RFC 5322 messages it holds, one ready-to-save `.eml` each, with a
//! suggested filename per message. Chat schema single-sourced from descriptor();
//! handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mbox: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_naming")]
    naming: String,
    #[serde(default)]
    message: i64,
    #[serde(default = "default_true")]
    unescape_from: bool,
    #[serde(default)]
    keep_postmark: bool,
}

fn default_output() -> String {
    "files".to_string()
}

fn default_naming() -> String {
    "index".to_string()
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mbox")
                .required()
                .describe("The mbox archive text to split. Messages are delimited by the classic `From ` postmark lines at the start of a line (the space after `From` is what distinguishes a postmark from a `From:` header); a single pasted RFC 5322 message with no postmark is treated as one message. Each message is sliced out verbatim, so headers, MIME structure and base64 attachments are never re-serialized."),
        )
        .param(
            Param::enumv("output", ["files", "list", "json", "eml"])
                .default("files")
                .describe("What to return: \"files\" (default) prints every message under a `===== NNN-name.eml (N bytes) =====` header, \"list\" prints a numbered index of filename, date, sender, subject and size without the message bodies, \"json\" returns an array of {index, filename, subject, from, date, bytes, eml}, and \"eml\" returns one raw message on its own (pair it with `message` to pick which)."),
        )
        .param(
            Param::enumv("naming", ["index", "subject", "date", "message-id"])
                .default("index")
                .describe("How each suggested filename is built: \"index\" (default) gives `001.eml`, \"subject\" gives `001-quarterly-report.eml`, \"date\" gives `001-2018-09-03-1000.eml` from the Date header, and \"message-id\" gives `001-a1-example-com.eml`. Every scheme keeps the 1-based index prefix so archive order and uniqueness survive; subjects are RFC 2047-decoded and slugged to portable ASCII."),
        )
        .param(
            Param::integer("message")
                .default(0)
                .min(0.0)
                .max(2000.0)
                .describe("Which single message to return, 1-based in archive order. The default 0 returns every message; set it with output=\"eml\" to pull one clean `.eml` out of the archive. A number past the end of the archive is an error that reports how many messages there are."),
        )
        .param(
            Param::boolean("unescape_from")
                .default(true)
                .describe("Undo the mboxo/mboxrd `>From ` body quoting that exporters add to body lines starting with `From `, restoring the original message text. On by default; turn it off to keep the archive bytes exactly as written."),
        )
        .param(
            Param::boolean("keep_postmark")
                .default(false)
                .describe("Keep the `From sender date` postmark separator line at the top of each split-out message. Off by default because a `.eml` file is a bare RFC 5322 message; turn it on to round-trip pieces back into an mbox."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MboxSplitter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mbox-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split an mbox archive into individual .eml messages",
    skill(
        description = "Split a multi-message mbox archive (Thunderbird, Apple Mail, Gmail Takeout) into the individual RFC 5322 messages it contains, each a ready-to-save .eml with a suggested filename. Messages are split on the classic `From ` postmark lines and sliced out verbatim, so headers, MIME parts and base64 attachments are preserved exactly; the postmark itself is dropped unless keep_postmark=true. Choose output=\"files\" for every message under a labelled header, \"list\" for a numbered index of filename/date/sender/subject/size, \"json\" for structured per-message records, or \"eml\" plus message=N to pull one raw message out. Filenames follow naming=index|subject|date|message-id, always index-prefixed so order and uniqueness survive, with RFC 2047 encoded-word subjects decoded. mboxo/mboxrd `>From ` body quoting is undone by default. A message with no postmark is treated as a single message; archives over 2000 messages are rejected. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MboxSplitter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mbox-splitter", |a: Args| {
            gizza_ai_mbox_splitter_core::run(
                &a.mbox,
                &a.output,
                &a.naming,
                a.message,
                a.unescape_from,
                a.keep_postmark,
            )
            .map_err(SkillError::InvalidArgs)
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
                    "mbox": { "type": "string", "description": "The mbox archive text to split. Messages are delimited by the classic `From ` postmark lines at the start of a line (the space after `From` is what distinguishes a postmark from a `From:` header); a single pasted RFC 5322 message with no postmark is treated as one message. Each message is sliced out verbatim, so headers, MIME structure and base64 attachments are never re-serialized." },
                    "output": { "type": "string", "enum": ["files", "list", "json", "eml"], "default": "files", "description": "What to return: \"files\" (default) prints every message under a `===== NNN-name.eml (N bytes) =====` header, \"list\" prints a numbered index of filename, date, sender, subject and size without the message bodies, \"json\" returns an array of {index, filename, subject, from, date, bytes, eml}, and \"eml\" returns one raw message on its own (pair it with `message` to pick which)." },
                    "naming": { "type": "string", "enum": ["index", "subject", "date", "message-id"], "default": "index", "description": "How each suggested filename is built: \"index\" (default) gives `001.eml`, \"subject\" gives `001-quarterly-report.eml`, \"date\" gives `001-2018-09-03-1000.eml` from the Date header, and \"message-id\" gives `001-a1-example-com.eml`. Every scheme keeps the 1-based index prefix so archive order and uniqueness survive; subjects are RFC 2047-decoded and slugged to portable ASCII." },
                    "message": { "type": "integer", "default": 0, "minimum": 0, "maximum": 2000, "description": "Which single message to return, 1-based in archive order. The default 0 returns every message; set it with output=\"eml\" to pull one clean `.eml` out of the archive. A number past the end of the archive is an error that reports how many messages there are." },
                    "unescape_from": { "type": "boolean", "default": true, "description": "Undo the mboxo/mboxrd `>From ` body quoting that exporters add to body lines starting with `From `, restoring the original message text. On by default; turn it off to keep the archive bytes exactly as written." },
                    "keep_postmark": { "type": "boolean", "default": false, "description": "Keep the `From sender date` postmark separator line at the top of each split-out message. Off by default because a `.eml` file is a bare RFC 5322 message; turn it on to round-trip pieces back into an mbox." }
                },
                "required": ["mbox"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
