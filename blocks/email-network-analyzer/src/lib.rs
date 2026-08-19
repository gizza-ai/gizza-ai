//! gizza-ai/email-network-analyzer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_top() -> f64 {
    10.0
}
fn default_min_messages() -> f64 {
    1.0
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    me: String,
    #[serde(default)]
    nodes: String,
    #[serde(default)]
    recipients: String,
    #[serde(default)]
    direction: String,
    #[serde(default = "default_top")]
    top: f64,
    #[serde(default = "default_min_messages")]
    min_messages: f64,
    #[serde(default)]
    exclude: String,
    #[serde(default)]
    self_loops: bool,
    #[serde(default)]
    since: String,
    #[serde(default)]
    until: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Raw email text: an mbox export (messages separated by a `From ` postmark line at column 0), a single .eml, or just the headers. Only From:/To:/Cc:/Bcc:/Date: are read; message bodies are ignored. Max 4 MiB, 20000 messages."),
        )
        .param(
            Param::string("me")
                .describe("Your own email address, e.g. \"alice@example.com\". Optional — when set, the report adds a personal section: how much you sent and received, your reciprocity ratio (received per sent), and who you mail / hear from most. Case-insensitive."),
        )
        .param(
            Param::enumv("nodes", ["address", "domain"])
                .default("address")
                .describe("What each node is: address (default) keeps one node per email address; domain collapses every address to the part after the @, giving an organisation-level graph."),
        )
        .param(
            Param::enumv("recipients", ["to", "to-cc", "to-cc-bcc"])
                .default("to-cc")
                .describe("Which recipient headers become edges: to (To: only), to-cc (To: plus Cc:, the default), or to-cc-bcc (also Bcc:, which is normally only present in your own sent mail)."),
        )
        .param(
            Param::enumv("direction", ["directed", "undirected"])
                .default("directed")
                .describe("directed (default) keeps sender -> recipient as its own link, so A->B and B->A are separate. undirected merges both ways into one pair link with the message counts added together."),
        )
        .param(
            Param::integer("top")
                .default(10)
                .min(1.0)
                .max(100.0)
                .describe("How many rows to show in each ranked list (top senders, recipients, correspondents, links, and the personal lists), 1 to 100. Default 10. Does not affect csv/json/graphml/dot, which always export the full graph."),
        )
        .param(
            Param::integer("min_messages")
                .default(1)
                .min(1.0)
                .max(10000.0)
                .describe("Minimum messages a link must carry to be kept, 1 to 10000. Default 1 (keep everything). Raise it to drop one-off contacts and leave only the regular correspondence."),
        )
        .param(
            Param::string("exclude")
                .describe("Comma-separated substrings; any address containing one is dropped from both ends of every edge, e.g. \"noreply,notifications@,mailer-daemon\". Matching is case-insensitive substring matching, so a bare domain like \"example.org\" excludes everyone at it."),
        )
        .param(
            Param::boolean("self_loops")
                .default(false)
                .describe("Keep links where sender and recipient are the same node (you Cc'ing yourself; in domain mode, all mail inside one company). Default false, which drops them and reports the count in the notes."),
        )
        .param(
            Param::string("since")
                .describe("Earliest message date to include, as YYYY-MM-DD, inclusive, e.g. \"2024-01-01\". Empty means no lower bound. While either bound is set, messages with no Date: header are skipped and counted in the notes."),
        )
        .param(
            Param::string("until")
                .describe("Latest message date to include, as YYYY-MM-DD, inclusive, e.g. \"2024-12-31\". Empty means no upper bound. Must not be earlier than `since`."),
        )
        .param(
            Param::enumv("format", ["report", "csv", "json", "graphml", "dot"])
                .default("report")
                .describe("Output format: report (readable ranked summary, default), csv (edge list: from,to,messages,first,last), json (summary plus every node and edge), graphml (weighted graph for Gephi/NetworkX), or dot (Graphviz source)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-network-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a sender-to-recipient email network graph with ranked senders, recipients, and links.",
    skill(
        description = "Turn raw email text into a communication network. Paste an mbox export, a single .eml, or just the headers as `input`; every message becomes edges from its From: address to each To:/Cc:/Bcc: recipient, weighted by message volume. Returns a ranked report: message/participant/link totals, the date span, top senders, top recipients, top correspondents (sent + received), and the heaviest links with their first/last dates. Set `me` to your own address for a personal section (who you mail most, who mails you most, and a reciprocity ratio). Use nodes='domain' for an organisation-level rollup, direction='undirected' to merge A->B with B->A, recipients to choose which headers count, since/until (YYYY-MM-DD, inclusive) to window the analysis, exclude to drop noreply/automated addresses, min_messages to prune one-off contacts, self_loops to keep self-addressed mail, and top to size the ranked lists. format='csv' returns an edge list, 'json' the full node/edge structure, 'graphml' a weighted graph for Gephi or NetworkX, and 'dot' Graphviz source. Runs entirely on the pasted text — no mailbox account or network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-network-analyzer", |a: Args| {
            gizza_ai_email_network_analyzer_core::analyze(
                &a.input,
                &a.me,
                &a.nodes,
                &a.recipients,
                &a.direction,
                a.top,
                a.min_messages,
                &a.exclude,
                a.self_loops,
                &a.since,
                &a.until,
                &a.format,
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
                    "input": { "type": "string", "description": "Raw email text: an mbox export (messages separated by a `From ` postmark line at column 0), a single .eml, or just the headers. Only From:/To:/Cc:/Bcc:/Date: are read; message bodies are ignored. Max 4 MiB, 20000 messages." },
                    "me": { "type": "string", "description": "Your own email address, e.g. \"alice@example.com\". Optional — when set, the report adds a personal section: how much you sent and received, your reciprocity ratio (received per sent), and who you mail / hear from most. Case-insensitive." },
                    "nodes": { "type": "string", "enum": ["address", "domain"], "default": "address", "description": "What each node is: address (default) keeps one node per email address; domain collapses every address to the part after the @, giving an organisation-level graph." },
                    "recipients": { "type": "string", "enum": ["to", "to-cc", "to-cc-bcc"], "default": "to-cc", "description": "Which recipient headers become edges: to (To: only), to-cc (To: plus Cc:, the default), or to-cc-bcc (also Bcc:, which is normally only present in your own sent mail)." },
                    "direction": { "type": "string", "enum": ["directed", "undirected"], "default": "directed", "description": "directed (default) keeps sender -> recipient as its own link, so A->B and B->A are separate. undirected merges both ways into one pair link with the message counts added together." },
                    "top": { "type": "integer", "default": 10, "minimum": 1, "maximum": 100, "description": "How many rows to show in each ranked list (top senders, recipients, correspondents, links, and the personal lists), 1 to 100. Default 10. Does not affect csv/json/graphml/dot, which always export the full graph." },
                    "min_messages": { "type": "integer", "default": 1, "minimum": 1, "maximum": 10000, "description": "Minimum messages a link must carry to be kept, 1 to 10000. Default 1 (keep everything). Raise it to drop one-off contacts and leave only the regular correspondence." },
                    "exclude": { "type": "string", "description": "Comma-separated substrings; any address containing one is dropped from both ends of every edge, e.g. \"noreply,notifications@,mailer-daemon\". Matching is case-insensitive substring matching, so a bare domain like \"example.org\" excludes everyone at it." },
                    "self_loops": { "type": "boolean", "default": false, "description": "Keep links where sender and recipient are the same node (you Cc'ing yourself; in domain mode, all mail inside one company). Default false, which drops them and reports the count in the notes." },
                    "since": { "type": "string", "description": "Earliest message date to include, as YYYY-MM-DD, inclusive, e.g. \"2024-01-01\". Empty means no lower bound. While either bound is set, messages with no Date: header are skipped and counted in the notes." },
                    "until": { "type": "string", "description": "Latest message date to include, as YYYY-MM-DD, inclusive, e.g. \"2024-12-31\". Empty means no upper bound. Must not be earlier than `since`." },
                    "format": { "type": "string", "enum": ["report", "csv", "json", "graphml", "dot"], "default": "report", "description": "Output format: report (readable ranked summary, default), csv (edge list: from,to,messages,first,last), json (summary plus every node and edge), graphml (weighted graph for Gephi/NetworkX), or dot (Graphviz source)." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
