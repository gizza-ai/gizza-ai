//! bot-traffic-filter core — pure, no wafer/wasm-bindgen deps.
//!
//! Classifies each entry of an access log or event list as **bot/crawler** vs
//! **human** by its user-agent string, strips the bot hits, and reports the
//! human-versus-bot split. Detection is a curated, case-insensitive match of
//! the user-agent against known crawler/agent tokens (search engines, AI
//! crawlers, SEO tools, monitoring probes, social-preview fetchers, generic
//! HTTP libraries/scripts, and headless browsers), plus the standard `bot` /
//! `crawl` / `spider` / `slurp` token heuristic and an empty-user-agent rule.
//!
//! Everything runs locally — no DNS, IP-range, or behavioural signals — so a
//! spoofed user-agent that claims to be Googlebot is classified by what it
//! declares. That limitation is stated on the page.

use std::fmt::Write as _;

const DEFAULT_LIMIT: u32 = 500;
pub const MAX_LIMIT: u32 = 10_000;

/// How to pull the user-agent out of each input line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Detect: a Combined-Log-Format line → the last quoted field is the UA;
    /// otherwise the whole line is treated as a bare user-agent string.
    Auto,
    /// Every line is a Combined-Log-Format access line; the UA is the last
    /// double-quoted field (empty if the line has no quoted fields).
    Combined,
    /// Every line IS a bare user-agent string (no log wrapper).
    Plain,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Format::Auto,
            "combined" | "access" | "clf" => Format::Combined,
            "plain" | "ua" | "user-agent" => Format::Plain,
            other => {
                return Err(format!(
                    "unknown format '{other}' — expected auto, combined, or plain"
                ))
            }
        })
    }
}

/// What to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A human-readable summary: totals, the human/bot split with percentages,
    /// a per-category breakdown, and the top bots.
    Report,
    /// A Markdown table, one row per hit (line #, class, category, bot, UA).
    Table,
    /// A JSON array, one object per hit.
    Json,
    /// CSV: header + one row per hit.
    Csv,
    /// Only the original lines classified as human (bots stripped).
    Humans,
    /// Only the original lines classified as bot.
    Bots,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" | "summary" => Output::Report,
            "table" => Output::Table,
            "json" => Output::Json,
            "csv" => Output::Csv,
            "humans" | "human" | "strip" => Output::Humans,
            "bots" | "bot" => Output::Bots,
            other => {
                return Err(format!(
                    "unknown output '{other}' — expected report, table, json, csv, humans, or bots"
                ))
            }
        })
    }
}

/// The classification of a single hit.
pub struct Hit<'a> {
    pub raw: &'a str,
    pub user_agent: String,
    pub is_bot: bool,
    /// A stable category slug: `human` for humans, `empty-ua` for a
    /// missing/blank UA, else one of the bot categories below.
    pub category: &'static str,
    /// The bot's display name (empty for humans).
    pub name: String,
}

/// A known-bot signature: a lowercase substring, its category, and a display
/// name. Ordered specific → generic; the FIRST match wins, so specific named
/// agents are attributed before the broad `bot`/`crawl`/`spider` fallback.
struct Sig {
    needle: &'static str,
    category: &'static str,
    name: &'static str,
}

macro_rules! sig {
    ($needle:expr, $cat:expr, $name:expr) => {
        Sig { needle: $needle, category: $cat, name: $name }
    };
}

// Curated from public user-agent conventions (paraphrased, not any vendor's
// proprietary list). Needles are lowercase; matched as case-insensitive
// substrings of the user-agent.
const SIGNATURES: &[Sig] = &[
    // ---- Search engines ----
    sig!("googlebot", "search-engine", "Googlebot"),
    sig!("google-inspectiontool", "search-engine", "Google-InspectionTool"),
    sig!("storebot-google", "search-engine", "Storebot-Google"),
    sig!("bingbot", "search-engine", "Bingbot"),
    sig!("bingpreview", "search-engine", "BingPreview"),
    sig!("adidxbot", "search-engine", "AdIdxBot"),
    sig!("duckduckbot", "search-engine", "DuckDuckBot"),
    sig!("duckduckgo", "search-engine", "DuckDuckGo"),
    sig!("baiduspider", "search-engine", "Baiduspider"),
    sig!("yandexbot", "search-engine", "YandexBot"),
    sig!("yandex", "search-engine", "Yandex"),
    sig!("sogou", "search-engine", "Sogou"),
    sig!("exabot", "search-engine", "Exabot"),
    sig!("seznambot", "search-engine", "SeznamBot"),
    sig!("applebot", "search-engine", "Applebot"),
    sig!("petalbot", "search-engine", "PetalBot"),
    sig!("slurp", "search-engine", "Yahoo! Slurp"),
    // ---- AI crawlers ----
    sig!("gptbot", "ai-crawler", "GPTBot"),
    sig!("oai-searchbot", "ai-crawler", "OAI-SearchBot"),
    sig!("chatgpt-user", "ai-crawler", "ChatGPT-User"),
    sig!("claudebot", "ai-crawler", "ClaudeBot"),
    sig!("claude-web", "ai-crawler", "Claude-Web"),
    sig!("claude-user", "ai-crawler", "Claude-User"),
    sig!("anthropic-ai", "ai-crawler", "Anthropic-AI"),
    sig!("perplexitybot", "ai-crawler", "PerplexityBot"),
    sig!("perplexity-user", "ai-crawler", "Perplexity-User"),
    sig!("google-extended", "ai-crawler", "Google-Extended"),
    sig!("ccbot", "ai-crawler", "CCBot"),
    sig!("bytespider", "ai-crawler", "Bytespider"),
    sig!("amazonbot", "ai-crawler", "Amazonbot"),
    sig!("cohere-ai", "ai-crawler", "cohere-ai"),
    sig!("diffbot", "ai-crawler", "Diffbot"),
    sig!("meta-externalagent", "ai-crawler", "Meta-ExternalAgent"),
    sig!("facebookbot", "ai-crawler", "FacebookBot"),
    sig!("imagesiftbot", "ai-crawler", "ImagesiftBot"),
    sig!("youbot", "ai-crawler", "YouBot"),
    sig!("timpibot", "ai-crawler", "Timpibot"),
    sig!("omgili", "ai-crawler", "Omgili"),
    // ---- SEO / marketing crawlers ----
    sig!("ahrefsbot", "seo-tool", "AhrefsBot"),
    sig!("semrushbot", "seo-tool", "SemrushBot"),
    sig!("mj12bot", "seo-tool", "MJ12bot"),
    sig!("dotbot", "seo-tool", "DotBot"),
    sig!("rogerbot", "seo-tool", "rogerbot"),
    sig!("blexbot", "seo-tool", "BLEXBot"),
    sig!("dataforseobot", "seo-tool", "DataForSeoBot"),
    sig!("screaming frog", "seo-tool", "Screaming Frog SEO Spider"),
    sig!("sitebulb", "seo-tool", "Sitebulb"),
    sig!("seokicks", "seo-tool", "SEOkicks"),
    // ---- Monitoring / performance probes ----
    sig!("uptimerobot", "monitoring", "UptimeRobot"),
    sig!("pingdom", "monitoring", "Pingdom"),
    sig!("statuscake", "monitoring", "StatusCake"),
    sig!("site24x7", "monitoring", "Site24x7"),
    sig!("newrelicpinger", "monitoring", "NewRelicPinger"),
    sig!("datadog", "monitoring", "Datadog"),
    sig!("gtmetrix", "monitoring", "GTmetrix"),
    sig!("chrome-lighthouse", "monitoring", "Chrome-Lighthouse"),
    sig!("lighthouse", "monitoring", "Lighthouse"),
    sig!("google-pagespeed", "monitoring", "Google-PageSpeed"),
    // ---- Social / link-preview fetchers ----
    sig!("facebookexternalhit", "social", "facebookexternalhit"),
    sig!("twitterbot", "social", "Twitterbot"),
    sig!("linkedinbot", "social", "LinkedInBot"),
    sig!("slackbot", "social", "Slackbot"),
    sig!("telegrambot", "social", "TelegramBot"),
    sig!("whatsapp", "social", "WhatsApp"),
    sig!("discordbot", "social", "Discordbot"),
    sig!("pinterestbot", "social", "Pinterestbot"),
    sig!("pinterest", "social", "Pinterest"),
    sig!("redditbot", "social", "redditbot"),
    sig!("embedly", "social", "Embedly"),
    sig!("skypeuripreview", "social", "SkypeUriPreview"),
    sig!("developers.google.com/+/web/snippet", "social", "Google Snippet"),
    // ---- Generic HTTP libraries / scripts ----
    sig!("python-requests", "library", "python-requests"),
    sig!("python-urllib", "library", "python-urllib"),
    sig!("aiohttp", "library", "aiohttp"),
    sig!("httpx", "library", "httpx"),
    sig!("scrapy", "library", "Scrapy"),
    sig!("mechanize", "library", "mechanize"),
    sig!("go-http-client", "library", "Go-http-client"),
    sig!("okhttp", "library", "okhttp"),
    sig!("axios", "library", "axios"),
    sig!("node-fetch", "library", "node-fetch"),
    sig!("got (https://github.com/sindresorhus/got)", "library", "got"),
    sig!("guzzlehttp", "library", "GuzzleHttp"),
    sig!("libwww-perl", "library", "libwww-perl"),
    sig!("java/", "library", "Java"),
    sig!("apache-httpclient", "library", "Apache-HttpClient"),
    sig!("restsharp", "library", "RestSharp"),
    sig!("httpie", "library", "HTTPie"),
    sig!("postmanruntime", "library", "PostmanRuntime"),
    sig!("insomnia", "library", "Insomnia"),
    sig!("curl/", "library", "curl"),
    sig!("wget/", "library", "Wget"),
    sig!("wget ", "library", "Wget"),
    // ---- Headless / automation browsers ----
    sig!("headlesschrome", "headless", "HeadlessChrome"),
    sig!("phantomjs", "headless", "PhantomJS"),
    sig!("puppeteer", "headless", "Puppeteer"),
    sig!("playwright", "headless", "Playwright"),
    sig!("selenium", "headless", "Selenium"),
    sig!("jsdom", "headless", "jsdom"),
    sig!("electron", "headless", "Electron"),
];

/// Broad token heuristic applied AFTER the named list: any user-agent still
/// unmatched that contains one of these is a generic/unknown bot.
const GENERIC_TOKENS: &[&str] = &["bot", "crawl", "spider", "slurp"];

/// Extract the last double-quoted field of a line (the UA in Combined Log
/// Format), or `None` if the line has fewer than two `"` characters.
fn last_quoted(line: &str) -> Option<String> {
    let quotes: Vec<usize> = line.match_indices('"').map(|(i, _)| i).collect();
    if quotes.len() >= 2 {
        let end = quotes[quotes.len() - 1];
        let start = quotes[quotes.len() - 2];
        Some(line[start + 1..end].to_string())
    } else {
        None
    }
}

fn extract_ua(line: &str, format: Format) -> String {
    match format {
        Format::Plain => line.trim().to_string(),
        Format::Combined => last_quoted(line).unwrap_or_default(),
        Format::Auto => last_quoted(line).unwrap_or_else(|| line.trim().to_string()),
    }
}

/// From a generic-token match, pull a readable bot name: the first
/// whitespace/`;`/`(`/`)`/`+`/`,`-delimited token that itself contains a bot
/// token, with any trailing `/version` stripped. Falls back to "Bot".
fn extract_generic_name(ua: &str) -> String {
    for tok in ua.split(|c: char| {
        c.is_whitespace() || matches!(c, ';' | '(' | ')' | '+' | ',')
    }) {
        let low = tok.to_ascii_lowercase();
        if GENERIC_TOKENS.iter().any(|t| low.contains(t)) {
            let base = tok.split('/').next().unwrap_or(tok).trim();
            if !base.is_empty() {
                return base.to_string();
            }
        }
    }
    "Bot".to_string()
}

/// Classify one user-agent string.
fn classify(ua: &str, empty_is_bot: bool) -> (bool, &'static str, String) {
    let trimmed = ua.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return if empty_is_bot {
            (true, "empty-ua", "(no user-agent)".to_string())
        } else {
            (false, "human", String::new())
        };
    }
    let low = trimmed.to_ascii_lowercase();
    for sig in SIGNATURES {
        if low.contains(sig.needle) {
            return (true, sig.category, sig.name.to_string());
        }
    }
    if GENERIC_TOKENS.iter().any(|t| low.contains(t)) {
        return (true, "other-bot", extract_generic_name(trimmed));
    }
    (false, "human", String::new())
}

/// Human-friendly category label for the report.
fn category_label(cat: &str) -> &'static str {
    match cat {
        "search-engine" => "Search engines",
        "ai-crawler" => "AI crawlers",
        "seo-tool" => "SEO tools",
        "monitoring" => "Monitoring probes",
        "social" => "Social / link previews",
        "library" => "HTTP libraries / scripts",
        "headless" => "Headless browsers",
        "other-bot" => "Other bots",
        "empty-ua" => "Missing user-agent",
        _ => "Other",
    }
}

/// Stable display order of bot categories in the report.
const CATEGORY_ORDER: &[&str] = &[
    "search-engine",
    "ai-crawler",
    "seo-tool",
    "monitoring",
    "social",
    "library",
    "headless",
    "other-bot",
    "empty-ua",
];

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// Filter and report bot traffic.
///
/// - `input`: raw text, one hit per line (blank lines are skipped).
/// - `format`/`output`: parsed by [`Format`]/[`Output`].
/// - `empty_is_bot`: treat a missing/`-` user-agent as a bot.
/// - `limit`: row cap for the table/json/csv/humans/bots outputs (0 → default
///   of 500; clamped to 1..=[`MAX_LIMIT`]). `report` always summarizes every line.
pub fn filter(
    input: &str,
    format: &str,
    output: &str,
    empty_is_bot: bool,
    limit: u32,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let output = Output::parse(output)?;
    let cap = if limit == 0 { DEFAULT_LIMIT } else { limit.min(MAX_LIMIT) } as usize;

    let hits: Vec<Hit> = input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|raw| {
            let user_agent = extract_ua(raw, format);
            let (is_bot, category, name) = classify(&user_agent, empty_is_bot);
            Hit { raw, user_agent, is_bot, category, name }
        })
        .collect();

    if hits.is_empty() {
        return Err("no log lines to analyze — paste at least one access-log line or user-agent".into());
    }

    match output {
        Output::Report => Ok(render_report(&hits)),
        Output::Table => Ok(render_table(&hits, cap)),
        Output::Json => Ok(render_json(&hits, cap)),
        Output::Csv => Ok(render_csv(&hits, cap)),
        Output::Humans => Ok(render_raw(&hits, cap, false)),
        Output::Bots => Ok(render_raw(&hits, cap, true)),
    }
}

fn pct(n: usize, total: usize) -> String {
    if total == 0 {
        "0.0".to_string()
    } else {
        format!("{:.1}", (n as f64) * 100.0 / (total as f64))
    }
}

fn render_report(hits: &[Hit]) -> String {
    let total = hits.len();
    let bots = hits.iter().filter(|h| h.is_bot).count();
    let humans = total - bots;

    let mut out = String::new();
    let _ = writeln!(out, "Bot traffic report");
    let _ = writeln!(out, "==================");
    let _ = writeln!(out, "Total hits:  {total}");
    let _ = writeln!(out, "Human:       {humans} ({}%)", pct(humans, total));
    let _ = writeln!(out, "Bot:         {bots} ({}%)", pct(bots, total));

    // Category breakdown (bot categories only), in stable order.
    let mut any_cat = false;
    for &cat in CATEGORY_ORDER {
        let count = hits.iter().filter(|h| h.is_bot && h.category == cat).count();
        if count > 0 {
            if !any_cat {
                let _ = writeln!(out, "\nBots by category:");
                any_cat = true;
            }
            let _ = writeln!(out, "  {:<24} {count}", category_label(cat));
        }
    }

    // Top bots by name (descending count, then name for stability).
    let mut names: Vec<(&str, usize)> = Vec::new();
    for h in hits.iter().filter(|h| h.is_bot) {
        let key = h.name.as_str();
        if let Some(e) = names.iter_mut().find(|(n, _)| *n == key) {
            e.1 += 1;
        } else {
            names.push((key, 1));
        }
    }
    names.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    if !names.is_empty() {
        let _ = writeln!(out, "\nTop bots:");
        for (name, count) in names.iter().take(10) {
            let _ = writeln!(out, "  {:<24} {count}", name);
        }
    }

    out.trim_end().to_string()
}

fn class_word(is_bot: bool) -> &'static str {
    if is_bot {
        "bot"
    } else {
        "human"
    }
}

fn render_table(hits: &[Hit], cap: usize) -> String {
    let total = hits.len();
    let bots = hits.iter().filter(|h| h.is_bot).count();
    let humans = total - bots;

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{total} hits · {humans} human ({}%) · {bots} bot ({}%)",
        pct(humans, total),
        pct(bots, total)
    );
    let _ = writeln!(out, "\n| # | Class | Category | Bot | User-Agent |");
    let _ = writeln!(out, "| --- | --- | --- | --- | --- |");
    for (i, h) in hits.iter().take(cap).enumerate() {
        let cat = if h.is_bot { h.category } else { "-" };
        let name = if h.name.is_empty() { "-" } else { h.name.as_str() };
        let ua = if h.user_agent.trim().is_empty() {
            "(none)".to_string()
        } else {
            h.user_agent.replace('|', "\\|")
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            i + 1,
            class_word(h.is_bot),
            cat,
            name,
            ua
        );
    }
    if hits.len() > cap {
        let _ = writeln!(out, "\n_Showing first {cap} of {} hits._", hits.len());
    }
    out.trim_end().to_string()
}

fn render_json(hits: &[Hit], cap: usize) -> String {
    let rows: Vec<serde_json::Value> = hits
        .iter()
        .take(cap)
        .enumerate()
        .map(|(i, h)| {
            serde_json::json!({
                "line": i + 1,
                "class": class_word(h.is_bot),
                "is_bot": h.is_bot,
                "category": if h.is_bot { h.category } else { "human" },
                "bot": if h.name.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(h.name.clone()) },
                "user_agent": h.user_agent,
            })
        })
        .collect();
    serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".to_string())
}

fn render_csv(hits: &[Hit], cap: usize) -> String {
    let mut out = String::from("line,class,category,bot,user_agent");
    for (i, h) in hits.iter().take(cap).enumerate() {
        let cat = if h.is_bot { h.category } else { "human" };
        let _ = write!(
            out,
            "\n{},{},{},{},{}",
            i + 1,
            class_word(h.is_bot),
            cat,
            csv_escape(&h.name),
            csv_escape(&h.user_agent)
        );
    }
    out
}

/// The original lines of one class, joined by newlines. `want_bots=true` keeps
/// bot lines; false keeps human lines (the "strip bots" output).
fn render_raw(hits: &[Hit], cap: usize, want_bots: bool) -> String {
    hits.iter()
        .filter(|h| h.is_bot == want_bots)
        .take(cap)
        .map(|h| h.raw)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMBINED: &str = concat!(
        "1.1.1.1 - - [26/Jul/2026:10:00:00 +0000] \"GET / HTTP/1.1\" 200 12 \"-\" \"Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36\"\n",
        "2.2.2.2 - - [26/Jul/2026:10:00:01 +0000] \"GET /robots.txt HTTP/1.1\" 200 55 \"-\" \"Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)\"\n",
        "3.3.3.3 - - [26/Jul/2026:10:00:02 +0000] \"GET /api HTTP/1.1\" 200 3 \"-\" \"python-requests/2.31.0\"",
    );

    #[test]
    fn report_split_counts_and_percentages() {
        let out = filter(COMBINED, "auto", "report", true, 0).unwrap();
        assert!(out.contains("Total hits:  3"), "{out}");
        assert!(out.contains("Human:       1 (33.3%)"), "{out}");
        assert!(out.contains("Bot:         2 (66.7%)"), "{out}");
        assert!(out.contains("Googlebot"), "{out}");
        assert!(out.contains("python-requests"), "{out}");
    }

    #[test]
    fn humans_output_strips_bot_lines() {
        let out = filter(COMBINED, "combined", "humans", true, 0).unwrap();
        // Only the first (human browser) line survives.
        assert_eq!(out.lines().count(), 1);
        assert!(out.starts_with("1.1.1.1"), "{out}");
    }

    #[test]
    fn bots_output_keeps_only_bot_lines() {
        let out = filter(COMBINED, "combined", "bots", true, 0).unwrap();
        assert_eq!(out.lines().count(), 2);
        assert!(out.contains("Googlebot"));
        assert!(out.contains("python-requests"));
    }

    #[test]
    fn plain_user_agent_list_classifies_each() {
        let uas = "curl/8.0.1\nGPTBot\nMozilla/5.0 (Macintosh) Safari/605.1";
        let out = filter(uas, "plain", "csv", true, 0).unwrap();
        assert!(out.contains("bot,library,curl"), "{out}");
        assert!(out.contains("bot,ai-crawler,GPTBot"), "{out}");
        assert!(out.contains("human,human,"), "{out}");
    }

    #[test]
    fn empty_user_agent_toggle() {
        // Combined line with a "-" user-agent field.
        let line = "8.8.8.8 - - [26/Jul/2026:10:00:00 +0000] \"GET / HTTP/1.1\" 200 1 \"-\" \"-\"";
        let bot = filter(line, "combined", "report", true, 0).unwrap();
        assert!(bot.contains("Bot:         1"), "{bot}");
        let human = filter(line, "combined", "report", false, 0).unwrap();
        assert!(human.contains("Human:       1"), "{human}");
    }

    #[test]
    fn generic_token_name_extraction() {
        let out = filter("Mozilla/5.0 (compatible; SomeRandomBot/3.0; +http://x)", "plain", "csv", true, 0).unwrap();
        assert!(out.contains("bot,other-bot,SomeRandomBot"), "{out}");
    }

    #[test]
    fn json_is_valid_array() {
        let out = filter(COMBINED, "auto", "json", true, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 3);
        assert_eq!(v[1]["bot"], "Googlebot");
        assert_eq!(v[1]["is_bot"], true);
    }

    #[test]
    fn limit_caps_rows() {
        let out = filter(COMBINED, "auto", "csv", true, 1).unwrap();
        // header + 1 row.
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn blank_input_errors() {
        let err = filter("\n  \n", "auto", "report", true, 0).unwrap_err();
        assert!(err.contains("no log lines"), "{err}");
    }

    #[test]
    fn bad_format_errors() {
        let err = filter("x", "nope", "report", true, 0).unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn bad_output_errors() {
        let err = filter("x", "auto", "nope", true, 0).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }
}
