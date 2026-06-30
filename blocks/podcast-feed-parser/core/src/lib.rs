//! gizza-ai/podcast-feed-parser core — parse a podcast RSS/XML (or Atom) feed
//! into a clean, structured episode list: titles, publish dates, durations, and
//! audio enclosure URLs. Pure-Rust (`quick-xml` + `serde_json` + `chrono`
//! parse-only); no wafer/wasm-bindgen deps, no clock, no network.
//!
//! Supported inputs:
//! - RSS 2.0 (`<rss><channel><item>…`), including the iTunes podcast namespace
//!   (`itunes:duration`, `itunes:author`, `itunes:image`, `itunes:season`,
//!   `itunes:episode`, `itunes:explicit`, `itunes:summary`).
//! - Atom 1.0 (`<feed><entry>…`) with `<link rel="enclosure">` audio.
//! - RSS 1.0 / RDF where `<item>` elements are siblings of `<channel>`.
//!
//! Namespace prefixes are matched by local name, so `itunes:duration` and a
//! plain `duration` are treated alike; where the iTunes and core vocabularies
//! collide (notably `<image>`) both shapes are handled.

use chrono::DateTime;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Serialize;

/// Episode sort order applied before the optional `limit` is taken.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    /// Keep the feed's original document order (most feeds list newest first).
    Feed,
    /// Sort newest publish date first; undated episodes sink to the end.
    Newest,
    /// Sort oldest publish date first; undated episodes sink to the end.
    Oldest,
}

impl Order {
    /// Parse the page/CLI/chat `order` string; unknown values fall back to feed order.
    pub fn parse(s: &str) -> Order {
        match s.trim().to_ascii_lowercase().as_str() {
            "newest" | "desc" | "newest_first" => Order::Newest,
            "oldest" | "asc" | "oldest_first" => Order::Oldest,
            _ => Order::Feed,
        }
    }
}

/// Options controlling parsing and output shape.
pub struct Options {
    /// Maximum number of episodes to return; 0 means all.
    pub limit: usize,
    /// Ordering applied before `limit`.
    pub order: Order,
    /// Include each episode's (plain-text) description/summary.
    pub include_descriptions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { limit: 0, order: Order::Feed, include_descriptions: false }
    }
}

/// Channel-level podcast metadata plus the parsed episode list.
#[derive(Serialize, Debug, PartialEq)]
pub struct Feed {
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub link: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub image: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub language: String,
    /// Number of episodes in `episodes` (after ordering + limit).
    pub episode_count: usize,
    pub episodes: Vec<Episode>,
}

/// A single parsed episode. Optional fields are omitted from JSON when absent.
#[derive(Serialize, Debug, PartialEq)]
pub struct Episode {
    pub title: String,
    /// Publish date normalised to RFC 3339, when the original date parsed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
    /// The original, unmodified publish-date string from the feed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_raw: Option<String>,
    /// Duration normalised to `HH:MM:SS`, or the raw value if unrecognised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    /// Duration in whole seconds, when it could be computed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<i64>,
    /// Direct media (enclosure) URL — the playable/downloadable audio.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
    /// MIME type of the audio enclosure (e.g. `audio/mpeg`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_type: Option<String>,
    /// Enclosure size in bytes, when declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_length_bytes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ---------------------------------------------------------------------------
// Minimal XML DOM (keeps qualified element/attribute names for namespace work).
// ---------------------------------------------------------------------------

struct El {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<El>,
    text: String,
}

impl El {
    fn new(name: String) -> Self {
        El { name, attrs: Vec::new(), children: Vec::new(), text: String::new() }
    }
    fn local(&self) -> &str {
        local_of(&self.name)
    }
    fn text_trim(&self) -> String {
        self.text.trim().to_string()
    }
    /// First child whose local name matches (case-insensitive).
    fn child_local(&self, name: &str) -> Option<&El> {
        self.children.iter().find(|c| c.local().eq_ignore_ascii_case(name))
    }
    /// All children whose local name matches.
    fn children_local<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a El> + 'a {
        self.children.iter().filter(move |c| c.local().eq_ignore_ascii_case(name))
    }
    /// Attribute value by local name (case-insensitive).
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| local_of(k).eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Local part of an `ns:local` qualified name.
fn local_of(qname: &str) -> &str {
    match qname.rsplit_once(':') {
        Some((_, local)) => local,
        None => qname,
    }
}

fn parse_dom(xml: &str) -> Result<El, String> {
    if xml.trim().is_empty() {
        return Err("input feed is empty".to_string());
    }
    let mut reader = Reader::from_str(xml);
    let config = reader.config_mut();
    config.trim_text(false);
    config.expand_empty_elements = false;

    let mut stack: Vec<El> = Vec::new();
    let mut root: Option<El> = None;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => {
                return Err(format!(
                    "XML parse error at position {}: {}",
                    reader.buffer_position(),
                    e
                ))
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let mut el = El::new(qname(e.name().as_ref()));
                read_attrs(&e, &mut el)?;
                stack.push(el);
            }
            Ok(Event::Empty(e)) => {
                let mut el = El::new(qname(e.name().as_ref()));
                read_attrs(&e, &mut el)?;
                attach(&mut stack, &mut root, el)?;
            }
            Ok(Event::End(_)) => {
                let el = stack.pop().ok_or_else(|| "unexpected closing tag".to_string())?;
                attach(&mut stack, &mut root, el)?;
            }
            Ok(Event::Text(t)) => {
                let txt = t.unescape().map_err(|e| e.to_string())?;
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            Ok(Event::CData(t)) => {
                let txt = String::from_utf8_lossy(&t.into_inner()).into_owned();
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&txt);
                }
            }
            // Comments, PIs, declarations, doctype: ignored.
            _ => {}
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err("unclosed XML element(s)".to_string());
    }
    root.ok_or_else(|| "no root element found".to_string())
}

fn qname(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

fn attach(stack: &mut Vec<El>, root: &mut Option<El>, el: El) -> Result<(), String> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(el);
        Ok(())
    } else if root.is_none() {
        *root = Some(el);
        Ok(())
    } else {
        Err("multiple root elements: XML must have exactly one root".to_string())
    }
}

fn read_attrs(e: &quick_xml::events::BytesStart, el: &mut El) -> Result<(), String> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| err.to_string())?;
        let key = qname(attr.key.as_ref());
        let val = attr.unescape_value().map_err(|err| err.to_string())?.into_owned();
        el.attrs.push((key, val));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Feed extraction.
// ---------------------------------------------------------------------------

/// Parse a podcast feed into the structured [`Feed`] model.
pub fn parse_feed(xml: &str, opt: &Options) -> Result<Feed, String> {
    let root = parse_dom(xml)?;
    let root_local = root.local().to_ascii_lowercase();

    // Locate the channel/feed metadata element and the per-episode element name.
    let (channel, item_name): (&El, &str) = if root_local == "feed" {
        (&root, "entry") // Atom
    } else {
        // RSS 2.0 / RDF: <channel> holds metadata; fall back to the root itself.
        (root.child_local("channel").unwrap_or(&root), "item")
    };

    // Items usually live under <channel>; in RSS 1.0/RDF they are root siblings.
    let mut item_els: Vec<&El> = channel.children_local(item_name).collect();
    if item_els.is_empty() {
        item_els = root.children_local(item_name).collect();
    }

    let title = first_child_text(channel, &["title"]).unwrap_or_default();
    let description =
        first_child_text(channel, &["description", "summary", "subtitle"]).unwrap_or_default();
    let author = channel_author(channel);
    let link = channel_link(channel);
    let image = channel_image(channel);
    let language = first_child_text(channel, &["language"]).unwrap_or_default();

    // (sort_ts, Episode) pairs so we can order by parsed date without leaking
    // the timestamp into the serialized output.
    let mut built: Vec<(Option<i64>, Episode)> =
        item_els.iter().map(|it| build_episode(it, opt)).collect();

    match opt.order {
        Order::Feed => {}
        Order::Newest => built.sort_by(|a, b| date_key(b.0).cmp(&date_key(a.0))),
        Order::Oldest => built.sort_by(|a, b| date_key(a.0).cmp(&date_key(b.0))),
    }

    let mut episodes: Vec<Episode> = built.into_iter().map(|(_, e)| e).collect();
    if opt.limit > 0 && episodes.len() > opt.limit {
        episodes.truncate(opt.limit);
    }

    Ok(Feed {
        title,
        description,
        author,
        link,
        image,
        language,
        episode_count: episodes.len(),
        episodes,
    })
}

/// Parse a podcast feed and pretty-print the [`Feed`] as JSON.
pub fn to_json(xml: &str, opt: &Options) -> Result<String, String> {
    let feed = parse_feed(xml, opt)?;
    serde_json::to_string_pretty(&feed).map_err(|e| e.to_string())
}

/// Sort key: undated episodes (None) map to the earliest possible instant so,
/// in a "newest first" sort, they fall to the very end; in "oldest first" they
/// lead — which is the natural place for items a feed left undated.
fn date_key(ts: Option<i64>) -> i64 {
    ts.unwrap_or(i64::MIN)
}

fn build_episode(item: &El, opt: &Options) -> (Option<i64>, Episode) {
    let title = first_child_text(item, &["title"]).unwrap_or_default();

    let published_raw = first_child_text(item, &["pubDate", "published", "date", "updated"]);
    let (published, sort_ts) = match &published_raw {
        Some(raw) => match parse_date(raw) {
            Some((iso, ts)) => (Some(iso), Some(ts)),
            None => (None, None),
        },
        None => (None, None),
    };

    let (duration, duration_seconds) = match first_child_text(item, &["duration"]) {
        Some(raw) => parse_duration(&raw),
        None => (None, None),
    };

    let (audio_url, audio_type, audio_length_bytes) = audio_enclosure(item);

    let guid = first_child_text(item, &["guid", "id"]);
    let link = item_link(item);
    let season = first_child_text(item, &["season"]).and_then(|s| s.trim().parse::<i64>().ok());
    let episode_no =
        first_child_text(item, &["episode"]).and_then(|s| s.trim().parse::<i64>().ok());
    let explicit = first_child_text(item, &["explicit"]).and_then(|s| parse_bool(&s));

    let description = if opt.include_descriptions {
        first_child_text(item, &["description", "summary", "subtitle", "content", "encoded"])
            .map(|s| collapse_ws(&s))
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    (
        sort_ts,
        Episode {
            title,
            published,
            published_raw,
            duration,
            duration_seconds,
            audio_url,
            audio_type,
            audio_length_bytes,
            guid,
            link,
            season,
            episode: episode_no,
            explicit,
            description,
        },
    )
}

/// First non-empty trimmed text among children matching any of `names` (by local name).
fn first_child_text(el: &El, names: &[&str]) -> Option<String> {
    for n in names {
        for c in el.children_local(n) {
            let t = c.text_trim();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn channel_author(ch: &El) -> String {
    // itunes:author / managingEditor / dc:creator are plain text.
    if let Some(t) = first_child_text(ch, &["author", "managingEditor", "creator"]) {
        return t;
    }
    // Atom <author><name>…</name></author>.
    if let Some(a) = ch.child_local("author") {
        if let Some(name) = a.child_local("name") {
            let t = name.text_trim();
            if !t.is_empty() {
                return t;
            }
        }
    }
    String::new()
}

fn channel_link(ch: &El) -> String {
    if let Some(l) = ch.child_local("link") {
        let t = l.text_trim();
        if !t.is_empty() {
            return t;
        }
    }
    // Atom: prefer rel="alternate", else first link with an href (not self).
    let mut fallback = String::new();
    for l in ch.children_local("link") {
        let rel = l.attr("rel").unwrap_or("alternate");
        if rel.eq_ignore_ascii_case("self") {
            continue;
        }
        if let Some(h) = l.attr("href") {
            if rel.eq_ignore_ascii_case("alternate") {
                return h.to_string();
            }
            if fallback.is_empty() {
                fallback = h.to_string();
            }
        }
    }
    fallback
}

fn channel_image(ch: &El) -> String {
    for img in ch.children_local("image") {
        // itunes:image href="…"
        if let Some(href) = img.attr("href") {
            if !href.is_empty() {
                return href.to_string();
            }
        }
        // <image><url>…</url></image>
        if let Some(u) = img.child_local("url") {
            let t = u.text_trim();
            if !t.is_empty() {
                return t;
            }
        }
        let t = img.text_trim();
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

/// Audio enclosure URL/type/size: RSS `<enclosure>` then Atom `<link rel="enclosure">`.
fn audio_enclosure(item: &El) -> (Option<String>, Option<String>, Option<i64>) {
    if let Some(enc) = item.child_local("enclosure") {
        let url = enc.attr("url").filter(|s| !s.is_empty()).map(|s| s.to_string());
        if url.is_some() {
            let typ = enc.attr("type").filter(|s| !s.is_empty()).map(|s| s.to_string());
            let len = enc.attr("length").and_then(|l| l.trim().parse::<i64>().ok());
            return (url, typ, len);
        }
    }
    for l in item.children_local("link") {
        if l.attr("rel").map(|r| r.eq_ignore_ascii_case("enclosure")).unwrap_or(false) {
            if let Some(h) = l.attr("href").filter(|s| !s.is_empty()) {
                let typ = l.attr("type").filter(|s| !s.is_empty()).map(|s| s.to_string());
                let len = l.attr("length").and_then(|x| x.trim().parse::<i64>().ok());
                return (Some(h.to_string()), typ, len);
            }
        }
    }
    (None, None, None)
}

fn item_link(item: &El) -> Option<String> {
    // RSS: <link>https://…</link> (text content).
    if let Some(l) = item.child_local("link") {
        let t = l.text_trim();
        if !t.is_empty() {
            return Some(t);
        }
    }
    // Atom: rel="alternate" (or any non-enclosure link with href).
    let mut fallback = None;
    for l in item.children_local("link") {
        let rel = l.attr("rel").unwrap_or("alternate");
        if rel.eq_ignore_ascii_case("enclosure") {
            continue;
        }
        if let Some(h) = l.attr("href") {
            if rel.eq_ignore_ascii_case("alternate") {
                return Some(h.to_string());
            }
            if fallback.is_none() {
                fallback = Some(h.to_string());
            }
        }
    }
    fallback
}

/// Parse an RFC 2822 (RSS `pubDate`) or RFC 3339 (Atom) date into
/// `(rfc3339_string, unix_seconds)`.
fn parse_date(raw: &str) -> Option<(String, i64)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc2822(raw) {
        return Some((dt.to_rfc3339(), dt.timestamp()));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some((dt.to_rfc3339(), dt.timestamp()));
    }
    None
}

/// Normalise an iTunes duration. Accepts whole seconds (`"3600"`), `H:M:S`, or
/// `M:S`. Returns `(Some("HH:MM:SS"), Some(seconds))` when understood, or the
/// raw string with no seconds when not.
fn parse_duration(raw: &str) -> (Option<String>, Option<i64>) {
    let raw = raw.trim();
    if raw.is_empty() {
        return (None, None);
    }
    if let Ok(secs) = raw.parse::<i64>() {
        if secs >= 0 {
            return (Some(fmt_hms(secs)), Some(secs));
        }
    }
    let parts: Vec<&str> = raw.split(':').collect();
    if parts.len() == 2 || parts.len() == 3 {
        let nums: Result<Vec<i64>, _> = parts.iter().map(|p| p.trim().parse::<i64>()).collect();
        if let Ok(nums) = nums {
            if nums.iter().all(|&n| n >= 0) {
                let secs = if nums.len() == 3 {
                    nums[0] * 3600 + nums[1] * 60 + nums[2]
                } else {
                    nums[0] * 60 + nums[1]
                };
                return (Some(fmt_hms(secs)), Some(secs));
            }
        }
    }
    (Some(raw.to_string()), None)
}

fn fmt_hms(secs: i64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" | "explicit" => Some(true),
        "no" | "false" | "0" | "clean" => Some(false),
        _ => None,
    }
}

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd">
  <channel>
    <title>The Example Show</title>
    <description>A demo podcast.</description>
    <link>https://example.com</link>
    <language>en-us</language>
    <itunes:author>Jane Host</itunes:author>
    <itunes:image href="https://example.com/cover.jpg"/>
    <item>
      <title>Episode 2: Newer</title>
      <pubDate>Wed, 15 Jun 2022 09:00:00 GMT</pubDate>
      <itunes:duration>1:02:03</itunes:duration>
      <enclosure url="https://example.com/ep2.mp3" type="audio/mpeg" length="48000000"/>
      <guid>ep-2</guid>
      <link>https://example.com/ep2</link>
      <itunes:season>1</itunes:season>
      <itunes:episode>2</itunes:episode>
      <itunes:explicit>no</itunes:explicit>
      <description>The second episode.</description>
    </item>
    <item>
      <title>Episode 1: Older</title>
      <pubDate>Wed, 01 Jun 2022 09:00:00 GMT</pubDate>
      <itunes:duration>3600</itunes:duration>
      <enclosure url="https://example.com/ep1.mp3" type="audio/mpeg" length="24000000"/>
      <guid>ep-1</guid>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn parses_channel_metadata() {
        let f = parse_feed(RSS, &Options::default()).unwrap();
        assert_eq!(f.title, "The Example Show");
        assert_eq!(f.description, "A demo podcast.");
        assert_eq!(f.author, "Jane Host");
        assert_eq!(f.link, "https://example.com");
        assert_eq!(f.language, "en-us");
        assert_eq!(f.image, "https://example.com/cover.jpg");
        assert_eq!(f.episode_count, 2);
    }

    #[test]
    fn parses_episode_fields() {
        let f = parse_feed(RSS, &Options::default()).unwrap();
        let ep = &f.episodes[0];
        assert_eq!(ep.title, "Episode 2: Newer");
        assert_eq!(ep.audio_url.as_deref(), Some("https://example.com/ep2.mp3"));
        assert_eq!(ep.audio_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(ep.audio_length_bytes, Some(48000000));
        assert_eq!(ep.duration.as_deref(), Some("01:02:03"));
        assert_eq!(ep.duration_seconds, Some(3723));
        assert_eq!(ep.guid.as_deref(), Some("ep-2"));
        assert_eq!(ep.link.as_deref(), Some("https://example.com/ep2"));
        assert_eq!(ep.season, Some(1));
        assert_eq!(ep.episode, Some(2));
        assert_eq!(ep.explicit, Some(false));
        assert_eq!(ep.published.as_deref(), Some("2022-06-15T09:00:00+00:00"));
        assert_eq!(ep.published_raw.as_deref(), Some("Wed, 15 Jun 2022 09:00:00 GMT"));
    }

    #[test]
    fn duration_seconds_form_normalises() {
        let f = parse_feed(RSS, &Options::default()).unwrap();
        assert_eq!(f.episodes[1].duration.as_deref(), Some("01:00:00"));
        assert_eq!(f.episodes[1].duration_seconds, Some(3600));
    }

    #[test]
    fn descriptions_off_by_default() {
        let f = parse_feed(RSS, &Options::default()).unwrap();
        assert_eq!(f.episodes[0].description, None);
    }

    #[test]
    fn descriptions_on_when_requested() {
        let opt = Options { include_descriptions: true, ..Options::default() };
        let f = parse_feed(RSS, &opt).unwrap();
        assert_eq!(f.episodes[0].description.as_deref(), Some("The second episode."));
        // Episode 1 has no description element → field omitted.
        assert_eq!(f.episodes[1].description, None);
    }

    #[test]
    fn order_oldest_first() {
        let opt = Options { order: Order::Oldest, ..Options::default() };
        let f = parse_feed(RSS, &opt).unwrap();
        assert_eq!(f.episodes[0].title, "Episode 1: Older");
        assert_eq!(f.episodes[1].title, "Episode 2: Newer");
    }

    #[test]
    fn order_newest_first() {
        let opt = Options { order: Order::Newest, ..Options::default() };
        let f = parse_feed(RSS, &opt).unwrap();
        assert_eq!(f.episodes[0].title, "Episode 2: Newer");
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let opt = Options { order: Order::Oldest, limit: 1, ..Options::default() };
        let f = parse_feed(RSS, &opt).unwrap();
        assert_eq!(f.episode_count, 1);
        assert_eq!(f.episodes[0].title, "Episode 1: Older");
    }

    #[test]
    fn minute_second_duration() {
        assert_eq!(parse_duration("10:30"), (Some("00:10:30".to_string()), Some(630)));
    }

    #[test]
    fn unrecognised_duration_kept_raw() {
        assert_eq!(parse_duration("about an hour"), (Some("about an hour".to_string()), None));
    }

    #[test]
    fn atom_feed_supported() {
        let atom = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Atom Cast</title>
  <subtitle>Atom-format podcast.</subtitle>
  <link rel="alternate" href="https://atom.example"/>
  <author><name>Atom Author</name></author>
  <entry>
    <title>Atom Episode</title>
    <published>2023-03-04T12:00:00Z</published>
    <id>urn:uuid:1</id>
    <link rel="alternate" href="https://atom.example/1"/>
    <link rel="enclosure" type="audio/mpeg" length="12345" href="https://atom.example/1.mp3"/>
  </entry>
</feed>"#;
        let f = parse_feed(atom, &Options::default()).unwrap();
        assert_eq!(f.title, "Atom Cast");
        assert_eq!(f.description, "Atom-format podcast.");
        assert_eq!(f.author, "Atom Author");
        assert_eq!(f.link, "https://atom.example");
        assert_eq!(f.episode_count, 1);
        let ep = &f.episodes[0];
        assert_eq!(ep.title, "Atom Episode");
        assert_eq!(ep.audio_url.as_deref(), Some("https://atom.example/1.mp3"));
        assert_eq!(ep.audio_type.as_deref(), Some("audio/mpeg"));
        assert_eq!(ep.audio_length_bytes, Some(12345));
        assert_eq!(ep.link.as_deref(), Some("https://atom.example/1"));
        assert_eq!(ep.guid.as_deref(), Some("urn:uuid:1"));
        assert_eq!(ep.published.as_deref(), Some("2023-03-04T12:00:00+00:00"));
    }

    #[test]
    fn cdata_title_decoded() {
        let xml = r#"<rss><channel><title>X</title>
          <item><title><![CDATA[Tom & Jerry]]></title></item></channel></rss>"#;
        let f = parse_feed(xml, &Options::default()).unwrap();
        assert_eq!(f.episodes[0].title, "Tom & Jerry");
    }

    #[test]
    fn pretty_json_is_valid() {
        let s = to_json(RSS, &Options::default()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["title"], "The Example Show");
        assert!(s.contains("\"episodes\""));
    }

    #[test]
    fn err_on_empty() {
        assert!(parse_feed("   ", &Options::default()).is_err());
    }

    #[test]
    fn err_on_malformed() {
        assert!(parse_feed("<rss><channel><item></channel></rss>", &Options::default()).is_err());
    }

    #[test]
    fn empty_feed_has_no_episodes() {
        let xml = r#"<rss><channel><title>Empty</title></channel></rss>"#;
        let f = parse_feed(xml, &Options::default()).unwrap();
        assert_eq!(f.episode_count, 0);
        assert!(f.episodes.is_empty());
    }

    #[test]
    fn order_parse_aliases() {
        assert_eq!(Order::parse("Newest"), Order::Newest);
        assert_eq!(Order::parse("oldest"), Order::Oldest);
        assert_eq!(Order::parse("whatever"), Order::Feed);
    }
}
