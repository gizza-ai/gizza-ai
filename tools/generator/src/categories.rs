//! Fixed category taxonomy for the `/tools/` namespace: every tool page is
//! assigned to one or more categories by tag / slug-keyword rules, and each
//! category gets a hub page at `/tools/<category-slug>/`.
//!
//! Categories are matched IN ORDER — a tool's first match is its PRIMARY
//! category (used for related-tool tie-breaking and hub ordering). A tool that
//! matches nothing falls back to the last category (`utilities`), so every
//! tool always lands in at least one category.
//!
//! Hubs live under the same `/tools/<slug>/` namespace as tool pages, so the
//! generator must fail the build on a category-slug/block-slug collision
//! (see [`check_slug_collisions`]).

use crate::meta::ToolMeta;

pub struct Category {
    pub slug: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    /// Exact-match tags (compared lowercased/trimmed).
    tags: &'static [&'static str],
    /// Slug keywords: a plain word matches one hyphen-separated slug segment;
    /// a keyword containing '-' matches as a substring of the whole slug
    /// (e.g. "data-uri" → data-uri-encode without also grabbing parse-uri).
    slug_words: &'static [&'static str],
}

/// The full taxonomy, in match order (first match = primary category).
/// `utilities` is last and rule-less: it only catches the fallback.
pub const CATEGORIES: &[Category] = &[
    Category {
        slug: "audio",
        title: "Audio tools",
        blurb: "Convert, trim, normalize and edit audio right in your browser — \
                MP3, WAV, OGG and more. Nothing is uploaded.",
        tags: &["audio"],
        slug_words: &["audio", "waveform"],
    },
    Category {
        slug: "video",
        title: "Video tools",
        blurb: "Compress, trim, resize, convert and subtitle video locally — \
                powered by in-browser FFmpeg, no upload required.",
        tags: &["video", "ffmpeg", "subtitle", "srt", "vtt", "webvtt", "subrip"],
        slug_words: &["video", "gif", "srt", "subtitle"],
    },
    Category {
        slug: "image",
        title: "Image & design tools",
        blurb: "Convert, resize, compress and recolor images and SVGs — plus \
                color palettes, gradients and contrast checks.",
        tags: &["image", "photo", "svg", "png", "webp"],
        slug_words: &["image", "photo", "svg", "color", "gradient", "mindmap", "vignette"],
    },
    Category {
        slug: "data",
        title: "Data tools",
        blurb: "CSV, JSON, YAML, XML and friends — convert, query, diff, \
                format and clean structured data.",
        tags: &["csv", "json", "yaml", "xml", "toml", "avro", "json query", "html table generator"],
        slug_words: &[
            "csv", "json", "jq", "jsonpath", "jsonata", "yaml", "xml", "xpath", "avro", "toml",
            "plist", "protobuf", "bencode", "opml", "serialize", "unserialize",
        ],
    },
    Category {
        slug: "documents",
        title: "Document tools",
        blurb: "Markdown, LaTeX, citations, slides and resumes — write, \
                convert and format documents.",
        tags: &["markdown", "latex", "bibliography", "citation", "pdf", "epub"],
        slug_words: &[
            "markdown", "latex", "bibtex", "citation", "resume", "slides", "document", "toc",
            "pdf", "epub",
        ],
    },
    Category {
        slug: "security",
        title: "Security & crypto tools",
        blurb: "Hashes, ciphers, JWTs, passwords, keys and threat-intel \
                helpers — everything runs offline in your browser.",
        tags: &[
            "cryptography", "crypto", "cipher", "hash", "checksum", "digest", "password", "2fa",
            "otp", "threat intelligence", "malware analysis", "key derivation", "kdf",
            "json web token", "jws", "security", "privacy", "stream cipher", "block cipher",
            "legacy cipher",
        ],
        slug_words: &[
            "hash", "hashes", "cipher", "encrypt", "decrypt", "jwt", "jwk", "password", "totp",
            "hotp", "otp", "otpauth", "pgp", "rsa", "ecdsa", "sm2", "sm4", "hmac", "cmac",
            "pbkdf2", "scrypt", "argon2", "hkdf", "evp", "bcrypt", "keccak", "sha1", "sha256",
            "sha3", "xor", "fernet", "bip39", "ja3", "ja4", "ioc", "ssh", "pem", "asn1", "luhn",
            "token", "safelink", "aes", "des",
        ],
    },
    Category {
        slug: "encoding",
        title: "Encoding tools",
        blurb: "Base64, hex, URL encoding, Unicode and other codecs — encode \
                and decode any format.",
        tags: &["encoding", "url encode", "morse code"],
        slug_words: &[
            "base32", "base58", "base62", "base85", "base64", "bech32", "hex", "binary", "codec",
            "unicode", "charset", "morse", "encoder", "encode", "byte", "lz", "data-uri",
        ],
    },
    Category {
        slug: "network",
        title: "Network & web tools",
        blurb: "IPs, CIDRs, DNS, URLs, HTTP and email — parse, build and \
                analyze network and web data.",
        tags: &["network", "dns", "http", "url", "ipv4", "ipv6", "query string"],
        slug_words: &[
            "ip", "ipv4", "ipv6", "cidr", "dns", "tcp", "udp", "tls", "http", "websocket", "url",
            "urls", "uri", "mac", "ethernet", "grpc", "magnet", "email", "eml", "domains", "link",
            "header", "query",
        ],
    },
    Category {
        slug: "time",
        title: "Date & time tools",
        blurb: "Timestamps, timezones, durations and cron schedules — convert \
                and calculate dates and times.",
        tags: &["time", "date", "timezone", "unix timestamp", "datetime", "cron expression"],
        slug_words: &[
            "date", "dates", "datetime", "time", "clock", "timezone", "timestamp", "cron",
            "seconds", "age", "hms",
        ],
    },
    Category {
        slug: "math",
        title: "Math & statistics tools",
        blurb: "Calculators, statistics, unit conversion and number \
                crunching — instant results, no spreadsheet needed.",
        tags: &[
            "math", "maths", "statistics", "standard deviation", "technical analysis",
            "trading indicator", "moving average", "z-score",
        ],
        slug_words: &[
            "calculator", "stats", "average", "sum", "percentage", "geometry", "matrix", "graph",
            "product", "chi", "distribution", "math", "unit", "outlier", "normalize",
        ],
    },
    Category {
        slug: "text",
        title: "Text tools",
        blurb: "Sort, dedupe, clean, count, split and transform text and \
                lists — the plain-text Swiss army knife.",
        tags: &["text", "word counter", "text tool", "nlp", "text analysis"],
        slug_words: &[
            "text", "line", "lines", "word", "words", "case", "whitespace", "quotes", "slugify",
            "truncate", "wrap", "unwrap", "split", "join", "sort", "dedupe", "duplicate",
            "replace", "censor", "substring", "string", "list", "todo", "task", "emoji",
            "readability", "summarize", "keywords", "chars", "tweet", "cleaner", "diff",
            "frequency", "count", "ansi",
        ],
    },
    Category {
        slug: "developer",
        title: "Developer tools",
        blurb: "Formatters, minifiers, regex, SQL, UUIDs, mock data and other \
                coding helpers — all client-side.",
        tags: &[
            "developer", "template engine", "seo", "page weight", "web performance", "regex",
            "html", "css", "javascript", "sql",
        ],
        slug_words: &[
            "regex", "sql", "css", "js", "javascript", "html", "uuid", "mock", "fake", "jinja2",
            "template", "typescript", "flask", "session", "php", "seo", "sitemap", "form",
            "syntax", "autoprefixer", "playground", "tree", "gzip",
        ],
    },
    Category {
        slug: "utilities",
        title: "Utilities",
        blurb: "Handy everyday tools that don't fit a single box — validators, \
                formatters and more.",
        tags: &[],
        slug_words: &[],
    },
];

/// A category together with its member tools, ready for hub rendering.
pub struct Hub<'a> {
    pub category: &'static Category,
    pub members: Vec<&'a ToolMeta>,
}

impl Category {
    fn matches(&self, meta: &ToolMeta) -> bool {
        let tag_hit = meta
            .tags
            .iter()
            .any(|t| self.tags.contains(&t.trim().to_lowercase().as_str()));
        tag_hit
            || self.slug_words.iter().any(|kw| {
                if kw.contains('-') {
                    meta.slug.contains(kw)
                } else {
                    meta.slug.split('-').any(|w| w == *kw)
                }
            })
    }
}

/// Every category the tool belongs to, in taxonomy order — never empty
/// (falls back to the last, rule-less `utilities` category).
pub fn categories_for(meta: &ToolMeta) -> Vec<&'static Category> {
    let matched: Vec<&'static Category> =
        CATEGORIES.iter().filter(|c| c.matches(meta)).collect();
    if matched.is_empty() {
        vec![CATEGORIES.last().expect("taxonomy is non-empty")]
    } else {
        matched
    }
}

/// The tool's primary category: its first match in taxonomy order.
pub fn primary_category(meta: &ToolMeta) -> &'static Category {
    categories_for(meta)[0]
}

/// Group tools into hubs, in taxonomy order, keeping only non-empty
/// categories. Member order preserves the input (slug-sorted) order, so hub
/// pages and `_hubs.json` are deterministic.
pub fn build_hubs(metas: &[ToolMeta]) -> Vec<Hub<'_>> {
    CATEGORIES
        .iter()
        .map(|category| Hub {
            category,
            members: metas
                .iter()
                .filter(|m| categories_for(m).iter().any(|c| c.slug == category.slug))
                .collect(),
        })
        .filter(|hub| !hub.members.is_empty())
        .collect()
}

/// Hubs and tool pages share the `/tools/<slug>/` namespace: a category slug
/// that equals any block slug would silently overwrite (or be overwritten by)
/// that tool's page, so the build must fail loudly instead.
pub fn check_slug_collisions<'a, I>(block_slugs: I) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str>,
{
    for slug in block_slugs {
        if CATEGORIES.iter().any(|c| c.slug == slug) {
            return Err(format!(
                "category slug '{slug}' collides with block '{slug}' — hub pages and tool \
                 pages share the /tools/ namespace; rename the category in categories.rs"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(slug: &str, tags: &[&str]) -> ToolMeta {
        let tags_toml = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ");
        ToolMeta::from_toml(&format!(
            r#"
slug          = "{slug}"
title         = "t"
description   = "d"
tags          = [{tags_toml}]
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "run"
output_label  = "o"
format        = "text"
"#
        ))
        .unwrap()
    }

    #[test]
    fn assigns_by_tag_and_by_slug_word() {
        // tag rule: "audio" tag → audio, even without an audio slug word
        let m = tool("waveform-image", &["audio", "png"]);
        let cats: Vec<&str> = categories_for(&m).iter().map(|c| c.slug).collect();
        assert_eq!(cats[0], "audio", "audio tag wins as primary (taxonomy order)");
        assert!(cats.contains(&"image"), "slug word 'image' adds image membership");

        // slug-word rule: hyphen-separated word match, not substring
        let m = tool("image-resize", &[]);
        assert_eq!(primary_category(&m).slug, "image");
    }

    #[test]
    fn hyphenated_keyword_matches_substring_only() {
        // "data-uri" (hyphenated) matches data-uri-encode as a substring…
        let m = tool("data-uri-encode", &[]);
        assert_eq!(primary_category(&m).slug, "encoding");
        // …while plain "uri" must NOT pull parse-uri into encoding: it lands
        // in network via its own word rule.
        let m = tool("parse-uri", &[]);
        assert_eq!(primary_category(&m).slug, "network");
    }

    #[test]
    fn multi_membership_keeps_taxonomy_order_and_first_is_primary() {
        // "extract-audio-from-video": audio (first) + video — primary is audio.
        let m = tool("extract-audio-from-video", &["audio", "video"]);
        let cats: Vec<&str> = categories_for(&m).iter().map(|c| c.slug).collect();
        assert_eq!(cats, vec!["audio", "video"]);
        assert_eq!(primary_category(&m).slug, "audio");
    }

    #[test]
    fn unmatched_tool_falls_back_to_utilities() {
        let m = tool("iban-validator", &["iban checker", "mod 97"]);
        let cats = categories_for(&m);
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].slug, "utilities");
    }

    #[test]
    fn tag_matching_is_case_insensitive_and_trimmed() {
        let m = tool("some-tool", &[" Audio "]);
        assert_eq!(primary_category(&m).slug, "audio");
    }

    #[test]
    fn assignment_is_deterministic() {
        let m = tool("hash-text", &["hash", "hex", "base64", "cryptography"]);
        let a: Vec<&str> = categories_for(&m).iter().map(|c| c.slug).collect();
        let b: Vec<&str> = categories_for(&m).iter().map(|c| c.slug).collect();
        assert_eq!(a, b);
        assert_eq!(a[0], "security", "hash tools are security-primary");
    }

    #[test]
    fn build_hubs_groups_in_taxonomy_order_and_drops_empty() {
        let metas = vec![
            tool("audio-convert", &["audio"]),
            tool("video-trim", &["video"]),
            tool("iban-validator", &[]),
        ];
        let hubs = build_hubs(&metas);
        let slugs: Vec<&str> = hubs.iter().map(|h| h.category.slug).collect();
        assert_eq!(slugs, vec!["audio", "video", "utilities"]);
        assert_eq!(hubs[0].members[0].slug, "audio-convert");
    }

    #[test]
    fn category_slug_collision_fails_the_build() {
        // simulated collision: a block named after a category slug
        let err = check_slug_collisions(["calculator", "audio"]).unwrap_err();
        assert!(err.contains("'audio'"), "collision names the offending slug: {err}");
        assert!(err.contains("/tools/"), "explains the shared namespace: {err}");
        // no collision → ok
        check_slug_collisions(["calculator", "hash-text"]).unwrap();
    }

    #[test]
    fn category_slugs_are_unique() {
        let mut slugs: Vec<&str> = CATEGORIES.iter().map(|c| c.slug).collect();
        slugs.sort_unstable();
        slugs.dedup();
        assert_eq!(slugs.len(), CATEGORIES.len(), "duplicate category slug");
    }

    /// Guard on the REAL tool population (skipped when blocks/ is absent,
    /// e.g. in a crate-only checkout): every tool must land somewhere, and
    /// the `utilities` fallback must stay small — a growing fallback means
    /// the match rules need new keywords, not that tools are uncategorizable.
    #[test]
    fn real_population_fallback_stays_small() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let blocks = root.join("blocks");
        if !blocks.is_dir() {
            return;
        }
        let metas = crate::collect_tool_metas(&blocks).expect("collect metas");
        assert!(!metas.is_empty());
        let metas: Vec<ToolMeta> = metas.into_iter().map(|(_, m)| m).collect();
        let hubs = build_hubs(&metas);
        for hub in &hubs {
            eprintln!("{:<12} {:>3} tools", hub.category.slug, hub.members.len());
        }
        let fallback_members: Vec<&str> = hubs
            .iter()
            .find(|h| h.category.slug == "utilities")
            .map_or_else(Vec::new, |h| h.members.iter().map(|m| m.slug.as_str()).collect());
        let fallback = fallback_members.len();
        eprintln!("fallback (utilities): {fallback} — {fallback_members:?}");
        assert!(
            fallback <= 40,
            "{fallback} tools fell through to the utilities fallback — tighten the \
             category match rules in categories.rs"
        );
    }
}
