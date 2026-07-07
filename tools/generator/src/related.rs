//! Related-tools ranking: for each tool, the top 5 others by shared-tag
//! count, rendered as internal links on the tool page and its markdown twin.
//!
//! Ordering is fully deterministic: score (shared normalized tags) descending,
//! then same-primary-category before different, then slug ascending. Tools
//! with zero shared tags still rank (category and slug break the tie), so
//! every page gets a full related section even for niche tag sets.

use std::collections::BTreeSet;

use crate::categories::primary_category;
use crate::meta::ToolMeta;

/// How many related tools each page links to.
pub const RELATED_COUNT: usize = 5;

fn normalized_tags(meta: &ToolMeta) -> BTreeSet<String> {
    meta.tags.iter().map(|t| t.trim().to_lowercase()).collect()
}

/// The top [`RELATED_COUNT`] tools related to `target`, excluding itself.
pub fn related_tools<'a>(target: &ToolMeta, all: &'a [ToolMeta]) -> Vec<&'a ToolMeta> {
    let target_tags = normalized_tags(target);
    let target_primary = primary_category(target).slug;
    let mut ranked: Vec<(usize, bool, &ToolMeta)> = all
        .iter()
        .filter(|m| m.slug != target.slug)
        .map(|m| {
            let shared = normalized_tags(m).intersection(&target_tags).count();
            let same_primary = primary_category(m).slug == target_primary;
            (shared, same_primary, m)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.slug.cmp(&b.2.slug))
    });
    ranked.into_iter().take(RELATED_COUNT).map(|(_, _, m)| m).collect()
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
    fn ranks_by_shared_tag_count_descending() {
        let all = vec![
            tool("audio-convert", &["audio", "convert", "mp3", "wav"]),
            tool("audio-compress", &["audio", "compress", "mp3"]),
            tool("audio-eq", &["audio", "equalizer"]),
            tool("video-trim", &["video", "trim"]),
        ];
        let related = related_tools(&all[0], &all);
        // audio-compress shares 2 tags (audio, mp3) > audio-eq's 1 > video-trim's 0
        let slugs: Vec<&str> = related.iter().map(|m| m.slug.as_str()).collect();
        assert_eq!(slugs, vec!["audio-compress", "audio-eq", "video-trim"]);
    }

    #[test]
    fn excludes_self_and_caps_at_five() {
        let mut all = vec![tool("audio-convert", &["audio"])];
        for i in 0..8 {
            all.push(tool(&format!("audio-{i}"), &["audio"]));
        }
        let related = related_tools(&all[0], &all);
        assert_eq!(related.len(), RELATED_COUNT);
        assert!(related.iter().all(|m| m.slug != "audio-convert"), "self excluded");
    }

    #[test]
    fn tie_breaks_same_primary_category_then_slug() {
        // Both share exactly one tag ("trim") with the target; the same-
        // primary-category tool (audio) must outrank the video tool, even
        // though "video-trim" < "z-audio-loop" alphabetically.
        let all = vec![
            tool("audio-fade", &["trim"]),
            tool("z-audio-loop", &["trim"]),
            tool("video-trim", &["trim"]),
        ];
        let related = related_tools(&all[0], &all);
        let slugs: Vec<&str> = related.iter().map(|m| m.slug.as_str()).collect();
        assert_eq!(slugs, vec!["z-audio-loop", "video-trim"]);

        // Pure slug tie-break: same score, same primary category.
        let all = vec![
            tool("audio-fade", &["audio"]),
            tool("audio-loop", &["audio"]),
            tool("audio-eq", &["audio"]),
        ];
        let related = related_tools(&all[0], &all);
        let slugs: Vec<&str> = related.iter().map(|m| m.slug.as_str()).collect();
        assert_eq!(slugs, vec!["audio-eq", "audio-loop"], "slug ascending on full tie");
    }

    #[test]
    fn tag_comparison_is_normalized() {
        let all = vec![
            tool("a-tool", &["Audio ", "MP3"]),
            tool("b-tool", &["audio", "mp3"]),
            tool("c-tool", &["unrelated"]),
        ];
        let related = related_tools(&all[0], &all);
        assert_eq!(related[0].slug, "b-tool", "case/whitespace-insensitive tag match");
    }

    #[test]
    fn ranking_is_deterministic() {
        let all = vec![
            tool("a", &["x"]),
            tool("b", &["x"]),
            tool("c", &["x"]),
            tool("d", &["y"]),
        ];
        let first: Vec<String> =
            related_tools(&all[0], &all).iter().map(|m| m.slug.clone()).collect();
        for _ in 0..3 {
            let again: Vec<String> =
                related_tools(&all[0], &all).iter().map(|m| m.slug.clone()).collect();
            assert_eq!(first, again);
        }
    }
}
