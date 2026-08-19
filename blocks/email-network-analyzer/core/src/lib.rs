//! gizza-ai/email-network-analyzer core — build a sender→recipient
//! communication graph from raw email text.
//!
//! Input is an mbox export (messages separated by a `From ` postmark) or a
//! single `.eml` / headers-only paste. Every message contributes one edge per
//! distinct recipient, weighted by message volume, plus per-participant sent /
//! received / degree counts. Output is a readable report, a CSV edge list,
//! structured JSON, GraphML (Gephi / NetworkX) or Graphviz DOT.
//!
//! Pure-Rust (`mail-parser`, `serde_json`), wasm-safe, no clock: the analysis
//! window comes from the data or from explicit `since`/`until` arguments, so
//! every run is deterministic.

use std::collections::{BTreeMap, HashMap, HashSet};

use mail_parser::{Address, MessageParser};
use serde::Serialize;

/// Hard cap on the pasted blob — this is a paste-sized tool, not a mailbox
/// indexer.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Hard cap on parsed messages.
pub const MAX_MESSAGES: usize = 20_000;
/// Hard cap on `top`.
pub const MAX_TOP: usize = 100;
/// Hard cap on `min_messages`.
pub const MAX_MIN_MESSAGES: usize = 10_000;

// ---------------------------------------------------------------------------
// options
// ---------------------------------------------------------------------------

/// Node granularity: one node per address, or one node per domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nodes {
    Address,
    Domain,
}

impl Nodes {
    pub fn parse(s: &str) -> Result<Nodes, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "address" => Ok(Nodes::Address),
            "domain" => Ok(Nodes::Domain),
            other => Err(format!(
                "unknown nodes \"{other}\" — use \"address\" or \"domain\""
            )),
        }
    }
}

/// Which recipient headers become edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recipients {
    To,
    ToCc,
    ToCcBcc,
}

impl Recipients {
    pub fn parse(s: &str) -> Result<Recipients, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "to" => Ok(Recipients::To),
            "" | "to-cc" => Ok(Recipients::ToCc),
            "to-cc-bcc" => Ok(Recipients::ToCcBcc),
            other => Err(format!(
                "unknown recipients \"{other}\" — use \"to\", \"to-cc\" or \"to-cc-bcc\""
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Recipients::To => "To",
            Recipients::ToCc => "To+Cc",
            Recipients::ToCcBcc => "To+Cc+Bcc",
        }
    }
}

/// Directed (who mailed whom) or undirected (who talks with whom).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Directed,
    Undirected,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "directed" => Ok(Direction::Directed),
            "undirected" => Ok(Direction::Undirected),
            other => Err(format!(
                "unknown direction \"{other}\" — use \"directed\" or \"undirected\""
            )),
        }
    }
}

/// Output serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Report,
    Csv,
    Json,
    Graphml,
    Dot,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(Format::Report),
            "csv" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            "graphml" => Ok(Format::Graphml),
            "dot" => Ok(Format::Dot),
            other => Err(format!(
                "unknown format \"{other}\" — use \"report\", \"csv\", \"json\", \"graphml\" or \"dot\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// parsing helpers
// ---------------------------------------------------------------------------

/// Split a raw mbox blob into individual RFC 5322 message blocks.
///
/// mbox delimits messages with a `From ` "postmark" line at column 0 (the space
/// after `From` is what distinguishes it from a `From:` header). The postmark
/// line itself is dropped. A blob with no postmark is treated as one message, so
/// a lone `.eml` (or a headers-only paste) still parses.
pub fn split_messages(raw: &str) -> Vec<String> {
    let mut msgs: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    for line in raw.split_inclusive('\n') {
        if line.starts_with("From ") {
            if started && !cur.trim().is_empty() {
                msgs.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            started = true;
            continue;
        }
        if started {
            cur.push_str(line);
        }
    }
    if started && !cur.trim().is_empty() {
        msgs.push(cur);
    }
    if msgs.is_empty() && !raw.trim().is_empty() {
        msgs.push(raw.to_string());
    }
    msgs
}

/// Lower-case + trim an address, dropping any stray angle brackets.
fn norm_addr(a: &str) -> String {
    a.trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_ascii_lowercase()
}

/// The domain part of an address, or the whole string when there is no `@`.
fn domain_of(addr: &str) -> String {
    match addr.rsplit_once('@') {
        Some((_, d)) if !d.is_empty() => d.to_string(),
        _ => addr.to_string(),
    }
}

/// Collect `(address, display-name)` pairs from one header's address list.
fn addr_list(addr: Option<&Address>) -> Vec<(String, Option<String>)> {
    match addr {
        None => Vec::new(),
        Some(a) => a
            .iter()
            .filter_map(|m| {
                let email = m.address()?;
                let email = norm_addr(email);
                if email.is_empty() || !email.contains('@') {
                    return None;
                }
                let name = m
                    .name()
                    .map(|n| n.split_whitespace().collect::<Vec<_>>().join(" "))
                    .filter(|n| !n.is_empty() && n.to_ascii_lowercase() != email);
                Some((email, name))
            })
            .collect(),
    }
}

/// Validate a `YYYY-MM-DD` filter bound.
fn parse_bound(s: &str, what: &str) -> Result<Option<String>, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let b = s.as_bytes();
    let shaped = b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if !shaped {
        return Err(format!(
            "{what} must be a date as YYYY-MM-DD (e.g. 2024-01-31), got \"{s}\""
        ));
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(format!("{what} \"{s}\" is not a real calendar date"));
    }
    Ok(Some(s.to_string()))
}

/// The `YYYY-MM-DD` part of a message's `Date:` header, if it has a usable one.
fn message_day(msg: &mail_parser::Message<'_>) -> Option<String> {
    let s = msg.date()?.to_rfc3339();
    if s.len() < 10 {
        return None;
    }
    let day = &s[..10];
    let b = day.as_bytes();
    let ok = b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit());
    if ok {
        Some(day.to_string())
    } else {
        None
    }
}

/// Parse the `exclude` list into lower-cased substrings.
fn parse_exclude(s: &str) -> Vec<String> {
    s.split([',', ';', '\n', ' ', '\t'])
        .map(|p| p.trim().to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

fn is_excluded(addr: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| addr.contains(p.as_str()))
}

// ---------------------------------------------------------------------------
// graph model
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct Node {
    sent: usize,
    received: usize,
    out: HashSet<String>,
    inn: HashSet<String>,
    name: Option<String>,
}

#[derive(Default, Clone)]
struct Edge {
    messages: usize,
    first: Option<String>,
    last: Option<String>,
}

impl Edge {
    fn touch(&mut self, day: Option<&String>) {
        self.messages += 1;
        if let Some(d) = day {
            if self.first.as_ref().is_none_or(|f| d < f) {
                self.first = Some(d.clone());
            }
            if self.last.as_ref().is_none_or(|l| d > l) {
                self.last = Some(d.clone());
            }
        }
    }
}

/// Skipped-message tallies, surfaced in the report notes.
#[derive(Default, Clone, Serialize)]
pub struct Skipped {
    pub unparseable: usize,
    pub no_sender: usize,
    pub no_recipients: usize,
    pub outside_window: usize,
    pub undated: usize,
    pub excluded: usize,
    pub self_loops: usize,
}

impl Skipped {
    fn is_empty(&self) -> bool {
        self.unparseable == 0
            && self.no_sender == 0
            && self.no_recipients == 0
            && self.outside_window == 0
            && self.undated == 0
            && self.excluded == 0
            && self.self_loops == 0
    }
}

// ---------------------------------------------------------------------------
// JSON shapes
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct JsonSummary {
    messages: usize,
    participants: usize,
    links: usize,
    nodes: &'static str,
    recipients: &'static str,
    direction: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_date: Option<String>,
    min_messages: usize,
    skipped: Skipped,
}

#[derive(Serialize)]
struct JsonNode {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    sent: usize,
    received: usize,
    total: usize,
    out_degree: usize,
    in_degree: usize,
    contacts: usize,
}

#[derive(Serialize)]
struct JsonEdge {
    from: String,
    to: String,
    messages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    first: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<String>,
}

#[derive(Serialize)]
struct JsonMe {
    address: String,
    found: bool,
    sent: usize,
    received: usize,
    contacts: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    reciprocity: Option<f64>,
    sent_to: Vec<JsonPair>,
    received_from: Vec<JsonPair>,
}

#[derive(Serialize)]
struct JsonPair {
    address: String,
    messages: usize,
}

#[derive(Serialize)]
struct JsonOut {
    summary: JsonSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    me: Option<JsonMe>,
    top_senders: Vec<JsonNode>,
    top_recipients: Vec<JsonNode>,
    top_correspondents: Vec<JsonNode>,
    top_links: Vec<JsonEdge>,
    nodes: Vec<JsonNode>,
    edges: Vec<JsonEdge>,
}

// ---------------------------------------------------------------------------
// analysis
// ---------------------------------------------------------------------------

struct Graph {
    nodes: BTreeMap<String, Node>,
    directed: BTreeMap<(String, String), Edge>,
    messages: usize,
    first_date: Option<String>,
    last_date: Option<String>,
    skipped: Skipped,
}

#[allow(clippy::too_many_arguments)]
fn build_graph(
    data: &str,
    nodes_mode: Nodes,
    recipients: Recipients,
    exclude: &[String],
    self_loops: bool,
    since: Option<&String>,
    until: Option<&String>,
) -> Result<Graph, String> {
    let mut g = Graph {
        nodes: BTreeMap::new(),
        directed: BTreeMap::new(),
        messages: 0,
        first_date: None,
        last_date: None,
        skipped: Skipped::default(),
    };
    let parser = MessageParser::default();
    let blocks = split_messages(data);
    if blocks.len() > MAX_MESSAGES {
        return Err(format!(
            "too many messages: {} (limit {MAX_MESSAGES}) — split the mbox before pasting",
            blocks.len()
        ));
    }

    for block in blocks {
        let msg = match parser.parse(block.as_bytes()) {
            Some(m) => m,
            None => {
                g.skipped.unparseable += 1;
                continue;
            }
        };
        let from = addr_list(msg.from());
        let (sender, sender_name) = match from.into_iter().next() {
            Some(f) => f,
            None => {
                g.skipped.no_sender += 1;
                continue;
            }
        };

        let day = message_day(&msg);
        if since.is_some() || until.is_some() {
            match day.as_ref() {
                None => {
                    g.skipped.undated += 1;
                    continue;
                }
                Some(d) => {
                    if since.is_some_and(|s| d < s) || until.is_some_and(|u| d > u) {
                        g.skipped.outside_window += 1;
                        continue;
                    }
                }
            }
        }

        if is_excluded(&sender, exclude) {
            g.skipped.excluded += 1;
            continue;
        }

        let mut raw_to: Vec<(String, Option<String>)> = addr_list(msg.to());
        if matches!(recipients, Recipients::ToCc | Recipients::ToCcBcc) {
            raw_to.extend(addr_list(msg.cc()));
        }
        if matches!(recipients, Recipients::ToCcBcc) {
            raw_to.extend(addr_list(msg.bcc()));
        }

        let sender_key = match nodes_mode {
            Nodes::Address => sender.clone(),
            Nodes::Domain => domain_of(&sender),
        };

        // Distinct recipient keys for THIS message (a person Cc'd twice counts
        // once), keeping the first display name seen.
        let mut targets: Vec<(String, Option<String>)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut dropped_self = 0usize;
        for (addr, name) in raw_to {
            if is_excluded(&addr, exclude) {
                continue;
            }
            let key = match nodes_mode {
                Nodes::Address => addr,
                Nodes::Domain => domain_of(&addr),
            };
            if key == sender_key && !self_loops {
                dropped_self += 1;
                continue;
            }
            if seen.insert(key.clone()) {
                targets.push((key, name));
            }
        }
        g.skipped.self_loops += dropped_self;

        if targets.is_empty() {
            g.skipped.no_recipients += 1;
            continue;
        }

        g.messages += 1;
        if let Some(d) = day.as_ref() {
            if g.first_date.as_ref().is_none_or(|f| d < f) {
                g.first_date = Some(d.clone());
            }
            if g.last_date.as_ref().is_none_or(|l| d > l) {
                g.last_date = Some(d.clone());
            }
        }

        {
            let n = g.nodes.entry(sender_key.clone()).or_default();
            n.sent += 1;
            if nodes_mode == Nodes::Address && n.name.is_none() {
                n.name = sender_name.clone();
            }
        }
        for (key, name) in targets {
            {
                let n = g.nodes.entry(key.clone()).or_default();
                n.received += 1;
                n.inn.insert(sender_key.clone());
                if nodes_mode == Nodes::Address && n.name.is_none() {
                    n.name = name;
                }
            }
            g.nodes
                .entry(sender_key.clone())
                .or_default()
                .out
                .insert(key.clone());
            g.directed
                .entry((sender_key.clone(), key))
                .or_default()
                .touch(day.as_ref());
        }
    }

    Ok(g)
}

fn node_json(id: &str, n: &Node) -> JsonNode {
    let contacts: HashSet<&String> = n.out.iter().chain(n.inn.iter()).collect();
    JsonNode {
        id: id.to_string(),
        name: n.name.clone(),
        sent: n.sent,
        received: n.received,
        total: n.sent + n.received,
        out_degree: n.out.len(),
        in_degree: n.inn.len(),
        contacts: contacts.len(),
    }
}

/// Build the sender→recipient graph and render it in the requested format.
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    data: &str,
    me: &str,
    nodes: &str,
    recipients: &str,
    direction: &str,
    top: f64,
    min_messages: f64,
    exclude: &str,
    self_loops: bool,
    since: &str,
    until: &str,
    format: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err(
            "input is empty — paste one or more emails (an .mbox export, a single .eml, or just the headers)"
                .into(),
        );
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes — the limit is {MAX_INPUT_BYTES} bytes ({} MB); split the mailbox first",
            data.len(),
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    let nodes_mode = Nodes::parse(nodes)?;
    let recipients = Recipients::parse(recipients)?;
    let direction = Direction::parse(direction)?;
    let format = Format::parse(format)?;

    if !top.is_finite() || top.fract() != 0.0 {
        return Err(format!("top must be a whole number, got {top}"));
    }
    let top = top as i64;
    if !(1..=MAX_TOP as i64).contains(&top) {
        return Err(format!("top must be between 1 and {MAX_TOP}, got {top}"));
    }
    let top = top as usize;

    if !min_messages.is_finite() || min_messages.fract() != 0.0 {
        return Err(format!(
            "min_messages must be a whole number, got {min_messages}"
        ));
    }
    let min_messages = min_messages as i64;
    if !(1..=MAX_MIN_MESSAGES as i64).contains(&min_messages) {
        return Err(format!(
            "min_messages must be between 1 and {MAX_MIN_MESSAGES}, got {min_messages}"
        ));
    }
    let min_messages = min_messages as usize;

    let since = parse_bound(since, "since")?;
    let until = parse_bound(until, "until")?;
    if let (Some(s), Some(u)) = (since.as_ref(), until.as_ref()) {
        if s > u {
            return Err(format!("since ({s}) is after until ({u})"));
        }
    }
    let exclude = parse_exclude(exclude);

    let g = build_graph(
        data,
        nodes_mode,
        recipients,
        &exclude,
        self_loops,
        since.as_ref(),
        until.as_ref(),
    )?;

    if g.messages == 0 {
        let mut why = String::from("no usable messages found");
        if g.skipped.no_sender > 0 && g.nodes.is_empty() && g.skipped.no_recipients == 0 {
            why.push_str(" — every block was missing a From: header; paste raw email text including headers");
        } else if g.skipped.outside_window > 0 || g.skipped.undated > 0 {
            why.push_str(" in the since/until window — widen the dates (messages with no Date: header are skipped when a window is set)");
        } else if g.skipped.excluded > 0 || g.skipped.no_recipients > 0 {
            why.push_str(" after filtering — relax exclude, or turn on self_loops if the sender is also the only recipient");
        } else {
            why.push_str(
                " — messages need a From: header and at least one To:/Cc: recipient address",
            );
        }
        return Err(why);
    }

    // Personal view is computed from the DIRECTED edges regardless of the
    // requested direction, so "who I mail" stays meaningful in undirected mode.
    let me_key = {
        let m = norm_addr(me);
        if m.is_empty() {
            None
        } else if nodes_mode == Nodes::Domain {
            Some(domain_of(&m))
        } else {
            Some(m)
        }
    };

    // Fold to the requested direction, then apply the weight threshold.
    let mut folded: BTreeMap<(String, String), Edge> = BTreeMap::new();
    for ((a, b), e) in &g.directed {
        let key = match direction {
            Direction::Directed => (a.clone(), b.clone()),
            Direction::Undirected => {
                if a <= b {
                    (a.clone(), b.clone())
                } else {
                    (b.clone(), a.clone())
                }
            }
        };
        let slot = folded.entry(key).or_default();
        slot.messages += e.messages;
        if let Some(d) = e.first.as_ref() {
            if slot.first.as_ref().is_none_or(|f| d < f) {
                slot.first = Some(d.clone());
            }
        }
        if let Some(d) = e.last.as_ref() {
            if slot.last.as_ref().is_none_or(|l| d > l) {
                slot.last = Some(d.clone());
            }
        }
    }

    let mut edges: Vec<JsonEdge> = folded
        .into_iter()
        .filter(|(_, e)| e.messages >= min_messages)
        .map(|((from, to), e)| JsonEdge {
            from,
            to,
            messages: e.messages,
            first: e.first,
            last: e.last,
        })
        .collect();
    edges.sort_by(|a, b| {
        b.messages
            .cmp(&a.messages)
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.to.cmp(&b.to))
    });

    let all_nodes: Vec<JsonNode> = g.nodes.iter().map(|(id, n)| node_json(id, n)).collect();

    let rank = |key: fn(&JsonNode) -> usize| -> Vec<JsonNode> {
        let mut v: Vec<JsonNode> = g.nodes.iter().map(|(id, n)| node_json(id, n)).collect();
        v.retain(|n| key(n) > 0);
        v.sort_by(|a, b| key(b).cmp(&key(a)).then_with(|| a.id.cmp(&b.id)));
        v.truncate(top);
        v
    };
    let top_senders = rank(|n| n.sent);
    let top_recipients = rank(|n| n.received);
    let top_correspondents = rank(|n| n.total);
    let top_links: Vec<JsonEdge> = edges
        .iter()
        .take(top)
        .map(|e| JsonEdge {
            from: e.from.clone(),
            to: e.to.clone(),
            messages: e.messages,
            first: e.first.clone(),
            last: e.last.clone(),
        })
        .collect();

    let me_view = me_key.as_ref().map(|key| {
        let node = g.nodes.get(key);
        let mut sent_to: Vec<JsonPair> = g
            .directed
            .iter()
            .filter(|((a, _), _)| a == key)
            .map(|((_, b), e)| JsonPair {
                address: b.clone(),
                messages: e.messages,
            })
            .collect();
        let mut received_from: Vec<JsonPair> = g
            .directed
            .iter()
            .filter(|((_, b), _)| b == key)
            .map(|((a, _), e)| JsonPair {
                address: a.clone(),
                messages: e.messages,
            })
            .collect();
        for v in [&mut sent_to, &mut received_from] {
            v.sort_by(|x, y| {
                y.messages
                    .cmp(&x.messages)
                    .then_with(|| x.address.cmp(&y.address))
            });
            v.truncate(top);
        }
        let sent = node.map(|n| n.sent).unwrap_or(0);
        let received = node.map(|n| n.received).unwrap_or(0);
        let contacts = node
            .map(|n| n.out.iter().chain(n.inn.iter()).collect::<HashSet<_>>().len())
            .unwrap_or(0);
        JsonMe {
            address: key.clone(),
            found: node.is_some(),
            sent,
            received,
            contacts,
            reciprocity: if sent > 0 {
                Some((received as f64 / sent as f64 * 100.0).round() / 100.0)
            } else {
                None
            },
            sent_to,
            received_from,
        }
    });

    let summary = JsonSummary {
        messages: g.messages,
        participants: g.nodes.len(),
        links: edges.len(),
        nodes: match nodes_mode {
            Nodes::Address => "address",
            Nodes::Domain => "domain",
        },
        recipients: match recipients {
            Recipients::To => "to",
            Recipients::ToCc => "to-cc",
            Recipients::ToCcBcc => "to-cc-bcc",
        },
        direction: match direction {
            Direction::Directed => "directed",
            Direction::Undirected => "undirected",
        },
        first_date: g.first_date.clone(),
        last_date: g.last_date.clone(),
        min_messages,
        skipped: g.skipped.clone(),
    };

    match format {
        Format::Csv => Ok(render_csv(&edges)),
        Format::Graphml => Ok(render_graphml(&edges, &all_nodes, direction)),
        Format::Dot => Ok(render_dot(&edges, direction)),
        Format::Json => {
            let out = JsonOut {
                summary,
                me: me_view,
                top_senders,
                top_recipients,
                top_correspondents,
                top_links,
                nodes: all_nodes,
                edges,
            };
            serde_json::to_string_pretty(&out).map_err(|e| format!("failed to serialize JSON: {e}"))
        }
        Format::Report => Ok(render_report(
            &summary,
            me_view.as_ref(),
            &top_senders,
            &top_recipients,
            &top_correspondents,
            &top_links,
            direction,
            recipients,
            nodes_mode,
            since.as_ref(),
            until.as_ref(),
        )),
    }
}

// ---------------------------------------------------------------------------
// renderers
// ---------------------------------------------------------------------------

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(edges: &[JsonEdge]) -> String {
    let mut out = String::from("from,to,messages,first,last\n");
    for e in edges {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_field(&e.from),
            csv_field(&e.to),
            e.messages,
            e.first.as_deref().unwrap_or(""),
            e.last.as_deref().unwrap_or("")
        ));
    }
    out
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_graphml(edges: &[JsonEdge], nodes: &[JsonNode], direction: Direction) -> String {
    // Only nodes that survive the edge threshold are exported, so the graph
    // loads clean in Gephi / NetworkX.
    let mut used: Vec<&str> = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    for e in edges {
        for id in [e.from.as_str(), e.to.as_str()] {
            if seen.insert(id) {
                used.push(id);
            }
        }
    }
    used.sort_unstable();
    let index: HashMap<&str, usize> = used.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let stats: HashMap<&str, &JsonNode> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<graphml xmlns=\"http://graphml.graphdrawing.org/xmlns\">\n");
    out.push_str("  <key id=\"label\" for=\"node\" attr.name=\"label\" attr.type=\"string\"/>\n");
    out.push_str("  <key id=\"sent\" for=\"node\" attr.name=\"sent\" attr.type=\"int\"/>\n");
    out.push_str("  <key id=\"received\" for=\"node\" attr.name=\"received\" attr.type=\"int\"/>\n");
    out.push_str("  <key id=\"weight\" for=\"edge\" attr.name=\"weight\" attr.type=\"int\"/>\n");
    out.push_str(&format!(
        "  <graph id=\"email-network\" edgedefault=\"{}\">\n",
        match direction {
            Direction::Directed => "directed",
            Direction::Undirected => "undirected",
        }
    ));
    for id in &used {
        let i = index[id];
        let (sent, received) = stats.get(id).map(|n| (n.sent, n.received)).unwrap_or((0, 0));
        out.push_str(&format!(
            "    <node id=\"n{i}\">\n      <data key=\"label\">{}</data>\n      <data key=\"sent\">{sent}</data>\n      <data key=\"received\">{received}</data>\n    </node>\n",
            xml_escape(id)
        ));
    }
    for (i, e) in edges.iter().enumerate() {
        out.push_str(&format!(
            "    <edge id=\"e{i}\" source=\"n{}\" target=\"n{}\">\n      <data key=\"weight\">{}</data>\n    </edge>\n",
            index[e.from.as_str()],
            index[e.to.as_str()],
            e.messages
        ));
    }
    out.push_str("  </graph>\n</graphml>\n");
    out
}

fn render_dot(edges: &[JsonEdge], direction: Direction) -> String {
    let (kind, arrow) = match direction {
        Direction::Directed => ("digraph", "->"),
        Direction::Undirected => ("graph", "--"),
    };
    let mut out = format!("{kind} email_network {{\n");
    out.push_str("  graph [overlap=false, splines=true];\n");
    out.push_str("  node [shape=box, fontsize=10];\n");
    for e in edges {
        out.push_str(&format!(
            "  \"{}\" {arrow} \"{}\" [weight={}, label=\"{}\"];\n",
            e.from.replace('"', "\\\""),
            e.to.replace('"', "\\\""),
            e.messages,
            e.messages
        ));
    }
    out.push_str("}\n");
    out
}

fn pad(s: &str, w: usize) -> String {
    let len = s.chars().count();
    if len >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - len))
    }
}

fn width_of<'a>(items: impl Iterator<Item = &'a str>) -> usize {
    items.map(|s| s.chars().count()).max().unwrap_or(0).min(48)
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    summary: &JsonSummary,
    me: Option<&JsonMe>,
    top_senders: &[JsonNode],
    top_recipients: &[JsonNode],
    top_correspondents: &[JsonNode],
    top_links: &[JsonEdge],
    direction: Direction,
    recipients: Recipients,
    nodes_mode: Nodes,
    since: Option<&String>,
    until: Option<&String>,
) -> String {
    let mut out = String::new();
    let unit = match nodes_mode {
        Nodes::Address => "addresses",
        Nodes::Domain => "domains",
    };
    let link_word = if summary.links == 1 { "link" } else { "links" };
    let msg_word = if summary.messages == 1 {
        "message"
    } else {
        "messages"
    };
    out.push_str("Email network\n");
    out.push_str(&format!(
        "  {} {msg_word}, {} {unit}, {} {link_word}\n",
        summary.messages, summary.participants, summary.links
    ));
    out.push_str(&format!(
        "  View: {} graph of {unit}, recipients = {}\n",
        summary.direction,
        recipients.label()
    ));
    match (summary.first_date.as_ref(), summary.last_date.as_ref()) {
        (Some(f), Some(l)) if f == l => out.push_str(&format!("  Dates: {f}\n")),
        (Some(f), Some(l)) => out.push_str(&format!("  Dates: {f} .. {l}\n")),
        _ => out.push_str("  Dates: not available (no Date: headers)\n"),
    }
    if since.is_some() || until.is_some() {
        out.push_str(&format!(
            "  Window: {} .. {}\n",
            since.map(|s| s.as_str()).unwrap_or("start"),
            until.map(|s| s.as_str()).unwrap_or("end")
        ));
    }
    if summary.min_messages > 1 {
        out.push_str(&format!(
            "  Links shown: {}+ messages\n",
            summary.min_messages
        ));
    }

    let w = width_of(
        top_senders
            .iter()
            .chain(top_recipients.iter())
            .chain(top_correspondents.iter())
            .map(|n| n.id.as_str()),
    );

    out.push_str("\nTop senders\n");
    if top_senders.is_empty() {
        out.push_str("  (none)\n");
    }
    for (i, n) in top_senders.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {}  {:>5} sent to {} {}\n",
            i + 1,
            pad(&n.id, w),
            n.sent,
            n.out_degree,
            if n.out_degree == 1 {
                "contact"
            } else {
                "contacts"
            }
        ));
    }

    out.push_str("\nTop recipients\n");
    if top_recipients.is_empty() {
        out.push_str("  (none)\n");
    }
    for (i, n) in top_recipients.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {}  {:>5} received from {} {}\n",
            i + 1,
            pad(&n.id, w),
            n.received,
            n.in_degree,
            if n.in_degree == 1 {
                "contact"
            } else {
                "contacts"
            }
        ));
    }

    out.push_str("\nTop correspondents (sent + received)\n");
    for (i, n) in top_correspondents.iter().enumerate() {
        out.push_str(&format!(
            "  {:>2}. {}  {:>5} total ({} sent, {} received, {} contacts)\n",
            i + 1,
            pad(&n.id, w),
            n.total,
            n.sent,
            n.received,
            n.contacts
        ));
    }

    let arrow = match direction {
        Direction::Directed => "->",
        Direction::Undirected => "--",
    };
    out.push_str(&format!(
        "\nTop links ({})\n",
        match direction {
            Direction::Directed => "sender -> recipient",
            Direction::Undirected => "pair, both ways combined",
        }
    ));
    if top_links.is_empty() {
        out.push_str("  (none above the message threshold)\n");
    }
    let lw = width_of(top_links.iter().map(|e| e.from.as_str()));
    for (i, e) in top_links.iter().enumerate() {
        let span = match (e.first.as_ref(), e.last.as_ref()) {
            (Some(f), Some(l)) if f == l => format!("  [{f}]"),
            (Some(f), Some(l)) => format!("  [{f} .. {l}]"),
            _ => String::new(),
        };
        out.push_str(&format!(
            "  {:>2}. {} {arrow} {}  {:>5}{span}\n",
            i + 1,
            pad(&e.from, lw),
            e.to,
            e.messages
        ));
    }

    if let Some(m) = me {
        out.push_str(&format!("\nYour network — {}\n", m.address));
        if !m.found {
            out.push_str("  Not found in the analysed messages (check the address, the window, or the exclude list).\n");
        } else {
            out.push_str(&format!(
                "  Sent {}, received {}, across {} {}.\n",
                m.sent,
                m.received,
                m.contacts,
                if m.contacts == 1 { "contact" } else { "contacts" }
            ));
            if let Some(r) = m.reciprocity {
                out.push_str(&format!("  Reciprocity: {r:.2} received per sent.\n"));
            }
            if !m.sent_to.is_empty() {
                out.push_str("  You mail most:\n");
                for p in &m.sent_to {
                    out.push_str(&format!("    {:>5}  {}\n", p.messages, p.address));
                }
            }
            if !m.received_from.is_empty() {
                out.push_str("  You hear most from:\n");
                for p in &m.received_from {
                    out.push_str(&format!("    {:>5}  {}\n", p.messages, p.address));
                }
            }
        }
    }

    if !summary.skipped.is_empty() {
        out.push_str("\nNotes\n");
        let s = &summary.skipped;
        for (n, what) in [
            (s.unparseable, "block(s) were not parseable as an email"),
            (s.no_sender, "message(s) had no From: address"),
            (
                s.no_recipients,
                "message(s) had no usable recipient after filtering",
            ),
            (s.outside_window, "message(s) fell outside the date window"),
            (
                s.undated,
                "message(s) had no Date: header while a window was set",
            ),
            (s.excluded, "message(s) had an excluded sender"),
            (s.self_loops, "self-addressed recipient(s) were dropped"),
        ] {
            if n > 0 {
                out.push_str(&format!("  - {n} {what}\n"));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MBOX: &str = "From alice@example.com Mon Jan  1 00:00:00 2024\r\n\
From: Alice <alice@example.com>\r\n\
To: bob@example.com\r\n\
Cc: carol@example.org\r\n\
Date: Tue, 2 Jan 2024 10:00:00 +0000\r\n\
Subject: kickoff\r\n\
\r\n\
hello\r\n\
\r\n\
From bob@example.com Mon Jan  1 00:00:00 2024\r\n\
From: Bob <bob@example.com>\r\n\
To: alice@example.com\r\n\
Date: Wed, 3 Jan 2024 09:00:00 +0000\r\n\
Subject: re: kickoff\r\n\
\r\n\
ack\r\n\
\r\n\
From alice@example.com Mon Jan  1 00:00:00 2024\r\n\
From: Alice <alice@example.com>\r\n\
To: bob@example.com\r\n\
Date: Fri, 5 Jan 2024 11:30:00 +0000\r\n\
Subject: status\r\n\
\r\n\
update\r\n";

    fn run(fmt: &str) -> String {
        analyze(MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "", "", fmt).unwrap()
    }

    #[test]
    fn happy_report_counts_messages_edges_and_top_senders() {
        let out = run("report");
        assert!(out.contains("3 messages, 3 addresses, 3 links"), "{out}");
        assert!(out.contains("alice@example.com -> bob@example.com"), "{out}");
        assert!(out.contains("Dates: 2024-01-02 .. 2024-01-05"), "{out}");
        // Alice sent 2, Bob 1 — Alice ranks first.
        let senders = out.split("Top senders").nth(1).unwrap();
        assert!(senders.trim_start().starts_with("1. alice@example.com"), "{senders}");
    }

    #[test]
    fn error_on_text_without_email_headers() {
        let err = analyze(
            "just some notes, no headers here", "", "address", "to-cc", "directed", 10.0, 1.0, "",
            false, "", "", "report",
        )
        .unwrap_err();
        assert!(err.contains("no usable messages"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = analyze(
            "   ", "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "", "", "report",
        )
        .unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn csv_is_an_edge_list_with_dates() {
        let csv = run("csv");
        assert!(csv.starts_with("from,to,messages,first,last\n"), "{csv}");
        assert!(
            csv.contains("alice@example.com,bob@example.com,2,2024-01-02,2024-01-05"),
            "{csv}"
        );
    }

    #[test]
    fn json_exposes_summary_nodes_and_edges() {
        let v: serde_json::Value = serde_json::from_str(&run("json")).unwrap();
        assert_eq!(v["summary"]["messages"], 3);
        assert_eq!(v["summary"]["participants"], 3);
        assert_eq!(v["summary"]["direction"], "directed");
        assert_eq!(v["edges"].as_array().unwrap().len(), 3);
        assert_eq!(v["edges"][0]["messages"], 2);
    }

    #[test]
    fn to_only_drops_the_cc_edge() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(MBOX, "", "address", "to", "directed", 10.0, 1.0, "", false, "", "", "json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["summary"]["participants"], 2);
        assert_eq!(v["edges"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn undirected_merges_both_directions() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX, "", "address", "to-cc", "undirected", 10.0, 1.0, "", false, "", "", "json",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["edges"].as_array().unwrap().len(), 2);
        assert_eq!(v["edges"][0]["from"], "alice@example.com");
        assert_eq!(v["edges"][0]["to"], "bob@example.com");
        assert_eq!(v["edges"][0]["messages"], 3);
    }

    #[test]
    fn domain_mode_collapses_addresses() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX, "", "domain", "to-cc", "directed", 10.0, 1.0, "", false, "", "", "json",
            )
            .unwrap(),
        )
        .unwrap();
        // example.com -> example.com is a self-loop and is dropped by default,
        // leaving only example.com -> example.org.
        assert_eq!(v["summary"]["participants"], 2);
        assert_eq!(v["edges"].as_array().unwrap().len(), 1);
        assert_eq!(v["edges"][0]["from"], "example.com");
        assert_eq!(v["edges"][0]["to"], "example.org");
    }

    #[test]
    fn domain_mode_with_self_loops_keeps_internal_traffic() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX, "", "domain", "to-cc", "directed", 10.0, 1.0, "", true, "", "", "json",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["edges"].as_array().unwrap().len(), 2);
        assert_eq!(v["summary"]["messages"], 3);
    }

    #[test]
    fn min_messages_prunes_one_off_links() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(MBOX, "", "address", "to-cc", "directed", 10.0, 2.0, "", false, "", "", "json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["edges"].as_array().unwrap().len(), 1);
        assert_eq!(v["summary"]["links"], 1);
    }

    #[test]
    fn window_filters_by_date() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "2024-01-03",
                "2024-01-04", "json",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["summary"]["messages"], 1);
        assert_eq!(v["summary"]["skipped"]["outside_window"], 2);
    }

    #[test]
    fn exclude_drops_matching_addresses() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "example.org", false, "", "",
                "json",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["summary"]["participants"], 2);
    }

    #[test]
    fn me_view_splits_sent_and_received() {
        let v: serde_json::Value = serde_json::from_str(
            &analyze(
                MBOX,
                "Alice@Example.com",
                "address",
                "to-cc",
                "undirected",
                10.0,
                1.0,
                "",
                false,
                "",
                "",
                "json",
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(v["me"]["address"], "alice@example.com");
        assert_eq!(v["me"]["found"], true);
        assert_eq!(v["me"]["sent"], 2);
        assert_eq!(v["me"]["received"], 1);
        assert_eq!(v["me"]["reciprocity"], 0.5);
        assert_eq!(v["me"]["sent_to"][0]["address"], "bob@example.com");
        assert_eq!(v["me"]["received_from"][0]["address"], "bob@example.com");
    }

    #[test]
    fn graphml_and_dot_export_the_same_edges() {
        let gml = run("graphml");
        assert!(gml.starts_with("<?xml version=\"1.0\""), "{gml}");
        assert!(gml.contains("edgedefault=\"directed\""), "{gml}");
        assert!(gml.contains("<data key=\"label\">alice@example.com</data>"), "{gml}");
        assert!(gml.contains("<data key=\"weight\">2</data>"), "{gml}");
        assert_eq!(gml.matches("<edge ").count(), 3);

        let dot = run("dot");
        assert!(dot.starts_with("digraph email_network {"), "{dot}");
        assert!(
            dot.contains("\"alice@example.com\" -> \"bob@example.com\" [weight=2, label=\"2\"];"),
            "{dot}"
        );
        let undirected = analyze(
            MBOX, "", "address", "to-cc", "undirected", 10.0, 1.0, "", false, "", "", "dot",
        )
        .unwrap();
        assert!(undirected.starts_with("graph email_network {"), "{undirected}");
        assert!(undirected.contains(" -- "), "{undirected}");
    }

    #[test]
    fn single_eml_without_postmark_still_parses() {
        let eml = "From: dev@example.net\nTo: ops@example.net, sre@example.net\nSubject: deploy\n\nbody\n";
        let v: serde_json::Value = serde_json::from_str(
            &analyze(eml, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "", "", "json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["summary"]["messages"], 1);
        assert_eq!(v["edges"].as_array().unwrap().len(), 2);
        assert!(v["summary"]["first_date"].is_null());
    }

    #[test]
    fn duplicate_recipient_in_one_message_counts_once() {
        let eml = "From: a@x.com\nTo: b@x.com, B@X.com\nCc: b@x.com\n\nhi\n";
        let v: serde_json::Value = serde_json::from_str(
            &analyze(eml, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "", "", "json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(v["edges"].as_array().unwrap().len(), 1);
        assert_eq!(v["edges"][0]["messages"], 1);
    }

    #[test]
    fn bad_enum_and_range_arguments_are_rejected() {
        let bad_format =
            analyze(MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "", "", "xml")
                .unwrap_err();
        assert!(bad_format.contains("unknown format"), "{bad_format}");

        let bad_top = analyze(
            MBOX, "", "address", "to-cc", "directed", 0.0, 1.0, "", false, "", "", "report",
        )
        .unwrap_err();
        assert!(bad_top.contains("top must be between 1 and 100"), "{bad_top}");

        let bad_min = analyze(
            MBOX, "", "address", "to-cc", "directed", 10.0, 0.0, "", false, "", "", "report",
        )
        .unwrap_err();
        assert!(bad_min.contains("min_messages must be between"), "{bad_min}");

        let bad_since = analyze(
            MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "01/02/2024", "",
            "report",
        )
        .unwrap_err();
        assert!(bad_since.contains("since must be a date"), "{bad_since}");

        let backwards = analyze(
            MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "2024-02-01",
            "2024-01-01", "report",
        )
        .unwrap_err();
        assert!(backwards.contains("is after until"), "{backwards}");
    }

    #[test]
    fn window_that_excludes_everything_explains_itself() {
        let err = analyze(
            MBOX, "", "address", "to-cc", "directed", 10.0, 1.0, "", false, "2025-01-01", "", "report",
        )
        .unwrap_err();
        assert!(err.contains("since/until window"), "{err}");
    }
}
