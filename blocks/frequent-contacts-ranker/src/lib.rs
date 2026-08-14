//! gizza-ai/frequent-contacts-ranker — rank the people in a mailbox export by
//! how often and how recently you exchange mail with them, so the top of the
//! list can be pasted into an address book / autocomplete list. Chat schema
//! single-sourced from descriptor(); handle() delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mbox: String,
    #[serde(default = "default_count")]
    count: String,
    #[serde(default = "default_true")]
    include_cc: bool,
    #[serde(default)]
    exclude: String,
    #[serde(default = "default_true")]
    skip_automated: bool,
    #[serde(default = "default_half_life")]
    half_life_days: f64,
    #[serde(default = "default_min_messages")]
    min_messages: i64,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_count() -> String {
    "both".to_string()
}

fn default_true() -> bool {
    true
}

fn default_half_life() -> f64 {
    180.0
}

fn default_min_messages() -> i64 {
    1
}

fn default_limit() -> i64 {
    25
}

fn default_sort() -> String {
    "score".to_string()
}

fn default_format() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mbox")
                .required()
                .describe("The mailbox text to rank. Paste an mbox export (Gmail Takeout, Thunderbird, Apple Mail) or one or more raw RFC 5322 messages. Messages are split on the classic `From ` postmark lines at the start of a line, and a single pasted message with no postmark counts as one message. Only the From/To/Cc/Bcc/Date headers are read — bodies and attachments are ignored. Up to 5000 messages per run."),
        )
        .param(
            Param::enumv("count", ["recipients", "senders", "both"])
                .default("both")
                .describe("Which side of the correspondence to rank. \"recipients\" counts only the people you addressed (To, plus Cc/Bcc when include_cc is on) — the \"people you email most\" list. \"senders\" counts only the people who wrote to you (From). \"both\" (default) folds the two directions into one ranking and still reports the per-direction counts in the `to` and `from` columns."),
        )
        .param(
            Param::boolean("include_cc")
                .default(true)
                .describe("Count Cc and Bcc recipients as well as To. On by default, because someone you routinely copy is a real contact. Turn it off to rank only people you addressed directly — useful when large Cc lists inflate the ranking. Ignored when count=\"senders\"."),
        )
        .param(
            Param::string("exclude")
                .describe("Addresses and domains to leave out, separated by commas, semicolons or spaces. An entry with an @ is matched as a whole address (`me@example.com`); an entry starting with @ or with no @ at all is matched as a domain (`@example.com` or `example.com`). Use it to drop your own address — otherwise you top your own ranking — and any internal or mailing-list domain. Empty by default."),
        )
        .param(
            Param::boolean("skip_automated")
                .default(true)
                .describe("Skip machine addresses so they cannot crowd out real people: local parts of noreply, no-reply, no_reply, donotreply, do-not-reply, do_not_reply, mailer-daemon, postmaster, bounce and bounces, including -/+ suffixed variants such as bounces-123@. On by default; turn it off to see newsletter and notification senders too (handy for finding what to unsubscribe from)."),
        )
        .param(
            Param::number("half_life_days")
                .default(180.0)
                .min(0.0)
                .max(3650.0)
                .describe("Recency half-life in days. Each message is weighted 0.5^(age in days / half-life), so at the default 180 a message from six months ago counts half as much as one from the newest day in the archive. Lower it (30–90) to favour who you talk to now; raise it to flatten the curve. 0 turns recency off entirely and ranks by raw message count. Ages are measured from the newest message in the paste, not today's date, so the same archive always ranks the same way."),
        )
        .param(
            Param::integer("min_messages")
                .default(1)
                .min(1.0)
                .max(1000.0)
                .describe("Drop anyone appearing in fewer than this many messages. The default 1 keeps everybody; raise it to 3 or 5 to cut one-off correspondents out of an autocomplete list. Filtering everything out is an error that reports how many addresses were found."),
        )
        .param(
            Param::integer("limit")
                .default(25)
                .min(0.0)
                .max(1000.0)
                .describe("How many ranked contacts to return. Default 25; set 0 to return every contact that passed the filters. The header line always reports how many contacts were found before the cut, so you can tell whether the list was truncated."),
        )
        .param(
            Param::enumv("sort", ["score", "messages", "recent", "name"])
                .default("score")
                .describe("Row order. \"score\" (default) uses the recency-weighted score, \"messages\" uses the raw message count, \"recent\" puts the most recently seen contact first, and \"name\" sorts A→Z by display name (falling back to the address). Ties always break on message count then address, so the order is reproducible."),
        )
        .param(
            Param::enumv("format", ["report", "list", "csv", "json"])
                .default("report")
                .describe("Output shape. \"report\" (default) is an aligned rank table under a summary line with the message total, date range and half-life. \"list\" is one `Name <address>` per line, ready to paste into an address book or autocomplete field. \"csv\" is an RFC 4180 table with rank,name,email,messages,to,from,first_seen,last_seen,score. \"json\" returns an object with the archive metadata plus a contacts array."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FrequentContactsRanker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/frequent-contacts-ranker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rank your most frequent email contacts by frequency and recency",
    skill(
        description = "Rank the people in a mailbox export by how often and how recently you exchange mail with them, so the top of the list can be pasted straight into an address book or autocomplete field. Paste an mbox archive (Gmail Takeout, Thunderbird, Apple Mail) or raw RFC 5322 messages; From/To/Cc/Bcc/Date headers are parsed, addresses are case-folded to one row per person, and the display name used most often is kept so the list reads as `Name <address>`. Each contact's score combines frequency with recency: every message is weighted 0.5^(age in days / half_life_days), measured from the newest message in the paste, so the ranking is deterministic and offline; half_life_days=0 ranks by raw message count instead. count=recipients|senders|both picks which side to rank, include_cc folds in Cc/Bcc, exclude drops your own address or whole domains, skip_automated (on by default) hides noreply/mailer-daemon style senders, min_messages sets a volume floor and limit caps the rows. format=report gives an aligned rank table with the message total and date range, list gives paste-ready `Name <address>` lines, csv gives rank,name,email,messages,to,from,first_seen,last_seen,score, and json adds the archive metadata. Up to 5000 messages per run. Runs locally — no mailbox account, no upload.",
        parameters = schema_json()
    ),
)]
impl FrequentContactsRanker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "frequent-contacts-ranker", |a: Args| {
            gizza_ai_frequent_contacts_ranker_core::run(
                &a.mbox,
                &a.count,
                a.include_cc,
                &a.exclude,
                a.skip_automated,
                a.half_life_days,
                a.min_messages,
                a.limit,
                &a.sort,
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
                    "mbox": { "type": "string", "description": "The mailbox text to rank. Paste an mbox export (Gmail Takeout, Thunderbird, Apple Mail) or one or more raw RFC 5322 messages. Messages are split on the classic `From ` postmark lines at the start of a line, and a single pasted message with no postmark counts as one message. Only the From/To/Cc/Bcc/Date headers are read — bodies and attachments are ignored. Up to 5000 messages per run." },
                    "count": { "type": "string", "enum": ["recipients", "senders", "both"], "default": "both", "description": "Which side of the correspondence to rank. \"recipients\" counts only the people you addressed (To, plus Cc/Bcc when include_cc is on) — the \"people you email most\" list. \"senders\" counts only the people who wrote to you (From). \"both\" (default) folds the two directions into one ranking and still reports the per-direction counts in the `to` and `from` columns." },
                    "include_cc": { "type": "boolean", "default": true, "description": "Count Cc and Bcc recipients as well as To. On by default, because someone you routinely copy is a real contact. Turn it off to rank only people you addressed directly — useful when large Cc lists inflate the ranking. Ignored when count=\"senders\"." },
                    "exclude": { "type": "string", "description": "Addresses and domains to leave out, separated by commas, semicolons or spaces. An entry with an @ is matched as a whole address (`me@example.com`); an entry starting with @ or with no @ at all is matched as a domain (`@example.com` or `example.com`). Use it to drop your own address — otherwise you top your own ranking — and any internal or mailing-list domain. Empty by default." },
                    "skip_automated": { "type": "boolean", "default": true, "description": "Skip machine addresses so they cannot crowd out real people: local parts of noreply, no-reply, no_reply, donotreply, do-not-reply, do_not_reply, mailer-daemon, postmaster, bounce and bounces, including -/+ suffixed variants such as bounces-123@. On by default; turn it off to see newsletter and notification senders too (handy for finding what to unsubscribe from)." },
                    "half_life_days": { "type": "number", "default": 180.0, "minimum": 0, "maximum": 3650, "description": "Recency half-life in days. Each message is weighted 0.5^(age in days / half-life), so at the default 180 a message from six months ago counts half as much as one from the newest day in the archive. Lower it (30–90) to favour who you talk to now; raise it to flatten the curve. 0 turns recency off entirely and ranks by raw message count. Ages are measured from the newest message in the paste, not today's date, so the same archive always ranks the same way." },
                    "min_messages": { "type": "integer", "default": 1, "minimum": 1, "maximum": 1000, "description": "Drop anyone appearing in fewer than this many messages. The default 1 keeps everybody; raise it to 3 or 5 to cut one-off correspondents out of an autocomplete list. Filtering everything out is an error that reports how many addresses were found." },
                    "limit": { "type": "integer", "default": 25, "minimum": 0, "maximum": 1000, "description": "How many ranked contacts to return. Default 25; set 0 to return every contact that passed the filters. The header line always reports how many contacts were found before the cut, so you can tell whether the list was truncated." },
                    "sort": { "type": "string", "enum": ["score", "messages", "recent", "name"], "default": "score", "description": "Row order. \"score\" (default) uses the recency-weighted score, \"messages\" uses the raw message count, \"recent\" puts the most recently seen contact first, and \"name\" sorts A→Z by display name (falling back to the address). Ties always break on message count then address, so the order is reproducible." },
                    "format": { "type": "string", "enum": ["report", "list", "csv", "json"], "default": "report", "description": "Output shape. \"report\" (default) is an aligned rank table under a summary line with the message total, date range and half-life. \"list\" is one `Name <address>` per line, ready to paste into an address book or autocomplete field. \"csv\" is an RFC 4180 table with rank,name,email,messages,to,from,first_seen,last_seen,score. \"json\" returns an object with the archive metadata plus a contacts array." }
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
