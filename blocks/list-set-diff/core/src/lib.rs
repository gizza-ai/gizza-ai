//! list-set-diff core — compare two lists as sets and report which items are
//! only in A, only in B, or shared by both, with counts. Pure compute, no
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

use std::collections::HashSet;

/// Maximum items processed per list. A generous cap that keeps the pure,
/// single-threaded comparison instant while guarding against pathological input.
pub const MAX_ITEMS: usize = 100_000;

/// Output ordering for each result section.
#[derive(Clone, Copy)]
enum Sort {
    /// Keep the order items first appear in their source list.
    Input,
    /// Ascending by item text (byte order).
    Asc,
    /// Descending by item text (byte order).
    Desc,
}

impl Sort {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "input" => Ok(Sort::Input),
            "asc" => Ok(Sort::Asc),
            "desc" => Ok(Sort::Desc),
            other => Err(format!(
                "invalid sort {other:?}: expected \"input\", \"asc\", or \"desc\""
            )),
        }
    }
}

/// Map a separator name to the character items are split on within each list.
fn separator_char(sep: &str) -> Result<char, String> {
    match sep {
        "" | "newline" | "line" => Ok('\n'),
        "comma" => Ok(','),
        "tab" => Ok('\t'),
        "semicolon" => Ok(';'),
        "pipe" => Ok('|'),
        "space" => Ok(' '),
        other => Err(format!(
            "invalid separator {other:?}: expected \"newline\", \"comma\", \"tab\", \"semicolon\", \"pipe\", or \"space\""
        )),
    }
}

/// One normalized item: the `display` text shown in the output and the `key`
/// used for equality (after case / leading-zero normalization).
struct Item {
    display: String,
    key: String,
}

/// Split `text` on `sep`, applying the trim / blank / normalization options, and
/// return the processed items in first-seen order. Errors if the item count
/// exceeds [`MAX_ITEMS`].
fn process(
    text: &str,
    sep: char,
    which: &str,
    trim: bool,
    ignore_blank: bool,
    ignore_case: bool,
    ignore_leading_zeros: bool,
) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for raw in text.split(sep) {
        let display = if trim { raw.trim() } else { raw };
        if ignore_blank && display.trim().is_empty() {
            continue;
        }
        let mut key = if ignore_case {
            display.to_lowercase()
        } else {
            display.to_string()
        };
        if ignore_leading_zeros {
            let stripped = key.trim_start_matches('0');
            key = if stripped.is_empty() {
                "0".to_string()
            } else {
                stripped.to_string()
            };
        }
        items.push(Item {
            display: display.to_string(),
            key,
        });
        if items.len() > MAX_ITEMS {
            return Err(format!(
                "list {which} has more than {MAX_ITEMS} items; reduce the input"
            ));
        }
    }
    Ok(items)
}

/// Order + optionally de-duplicate a result section's display strings.
fn finalize(items: Vec<Item>, dedupe: bool, sort: Sort) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(items.len());
    if dedupe {
        let mut seen: HashSet<String> = HashSet::new();
        for it in items {
            if seen.insert(it.key) {
                out.push(it.display);
            }
        }
    } else {
        out.extend(items.into_iter().map(|it| it.display));
    }
    match sort {
        Sort::Input => {}
        Sort::Asc => out.sort(),
        Sort::Desc => {
            out.sort();
            out.reverse();
        }
    }
    out
}

/// Render one titled section: a header with the displayed count, then the items
/// one per line (or `(none)` when empty).
fn render_section(title: &str, items: &[String]) -> String {
    let body = if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join("\n")
    };
    format!("{title} ({}):\n{body}", items.len())
}

/// Compare `list_a` and `list_b` as sets and return a human-readable report with
/// the only-in-A, only-in-B, and shared items plus a totals summary line.
///
/// - `separator` (`newline` default | `comma` | `tab` | `semicolon` | `pipe` |
///   `space`): how each list's items are split.
/// - `ignore_case`: compare case-insensitively (default false).
/// - `trim`: strip leading/trailing whitespace from each item (default true).
/// - `ignore_blank`: drop empty items (default true).
/// - `dedupe`: collapse repeated items within each section — set semantics
///   (default true).
/// - `ignore_leading_zeros`: treat e.g. `007` and `7` as equal (default false).
/// - `sort` (`input` default | `asc` | `desc`): ordering of each section.
///
/// Counts on the `Totals:` line are always unique-set based (so
/// `union = only-A + only-B + in-both`); section headers show the number of
/// lines actually displayed.
#[allow(clippy::too_many_arguments)]
pub fn diff(
    list_a: &str,
    list_b: &str,
    separator: &str,
    ignore_case: bool,
    trim: bool,
    ignore_blank: bool,
    dedupe: bool,
    ignore_leading_zeros: bool,
    sort: &str,
) -> Result<String, String> {
    let sep = separator_char(separator)?;
    let sort = Sort::parse(sort)?;

    let a = process(
        list_a, sep, "A", trim, ignore_blank, ignore_case, ignore_leading_zeros,
    )?;
    let b = process(
        list_b, sep, "B", trim, ignore_blank, ignore_case, ignore_leading_zeros,
    )?;

    let a_keys: HashSet<&str> = a.iter().map(|it| it.key.as_str()).collect();
    let b_keys: HashSet<&str> = b.iter().map(|it| it.key.as_str()).collect();

    let take = |src: &[Item], want_in: &HashSet<&str>, present: bool| -> Vec<Item> {
        src.iter()
            .filter(|it| want_in.contains(it.key.as_str()) == present)
            .map(|it| Item {
                display: it.display.clone(),
                key: it.key.clone(),
            })
            .collect()
    };

    let only_a = finalize(take(&a, &b_keys, false), dedupe, sort);
    let only_b = finalize(take(&b, &a_keys, false), dedupe, sort);
    let both = finalize(take(&a, &b_keys, true), dedupe, sort);

    // Unique-set counts for the summary line (union = A-only + B-only + both).
    let only_a_count = a_keys.difference(&b_keys).count();
    let only_b_count = b_keys.difference(&a_keys).count();
    let shared_count = a_keys.intersection(&b_keys).count();
    let union_count = only_a_count + only_b_count + shared_count;

    Ok(format!(
        "{}\n\n{}\n\n{}\n\nTotals: A={} · B={} · only in A={} · only in B={} · in both={} · union={}",
        render_section("Only in A", &only_a),
        render_section("Only in B", &only_b),
        render_section("In both", &both),
        a.len(),
        b.len(),
        only_a_count,
        only_b_count,
        shared_count,
        union_count,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convenience wrapper with the tool's defaults.
    fn d(a: &str, b: &str) -> String {
        diff(a, b, "newline", false, true, true, true, false, "input").unwrap()
    }

    #[test]
    fn basic_three_way_split_with_counts() {
        let out = d("apple\nbanana\ncherry", "banana\ncherry\ndate");
        assert_eq!(
            out,
            "Only in A (1):\napple\n\n\
             Only in B (1):\ndate\n\n\
             In both (2):\nbanana\ncherry\n\n\
             Totals: A=3 · B=3 · only in A=1 · only in B=1 · in both=2 · union=4"
        );
    }

    #[test]
    fn empty_section_shows_none() {
        // Everything shared → both only-in sections are empty.
        let out = d("x\ny", "y\nx");
        assert!(out.contains("Only in A (0):\n(none)"), "got: {out}");
        assert!(out.contains("Only in B (0):\n(none)"), "got: {out}");
        assert!(out.contains("In both (2):"), "got: {out}");
        assert!(out.contains("union=2"), "got: {out}");
    }

    #[test]
    fn ignore_case_matches_across_case() {
        let out = diff(
            "Alice@x.com\nBob@x.com",
            "bob@x.com\ncarol@x.com",
            "newline",
            true,
            true,
            true,
            true,
            false,
            "input",
        )
        .unwrap();
        // Bob is shared (case-insensitive); display keeps List A's original form.
        assert!(out.contains("In both (1):\nBob@x.com"), "got: {out}");
        assert!(out.contains("Only in A (1):\nAlice@x.com"), "got: {out}");
        assert!(out.contains("Only in B (1):\ncarol@x.com"), "got: {out}");
    }

    #[test]
    fn comma_separator_and_asc_sort() {
        let out = diff(
            "delta,alpha,charlie", "alpha,bravo", "comma", false, true, true, true, false, "asc",
        )
        .unwrap();
        assert!(out.contains("Only in A (2):\ncharlie\ndelta"), "got: {out}");
        assert!(out.contains("Only in B (1):\nbravo"), "got: {out}");
        assert!(out.contains("In both (1):\nalpha"), "got: {out}");
    }

    #[test]
    fn dedupe_collapses_repeats_within_a_list() {
        // A has "x" twice; with dedupe on it appears once.
        let deduped = d("x\nx\ny", "y");
        assert!(deduped.contains("Only in A (1):\nx"), "got: {deduped}");
        // With dedupe off, both occurrences are listed.
        let kept =
            diff("x\nx\ny", "y", "newline", false, true, true, false, false, "input").unwrap();
        assert!(kept.contains("Only in A (2):\nx\nx"), "got: {kept}");
        // Totals stay unique-set based regardless of dedupe.
        assert!(kept.contains("only in A=1"), "got: {kept}");
    }

    #[test]
    fn ignore_leading_zeros_matches_numeric_ids() {
        let out = diff(
            "007\n042", "7\n99", "newline", false, true, true, true, true, "input",
        )
        .unwrap();
        // 007 == 7 → shared; 042 only in A; 99 only in B.
        assert!(out.contains("In both (1):\n007"), "got: {out}");
        assert!(out.contains("Only in A (1):\n042"), "got: {out}");
        assert!(out.contains("Only in B (1):\n99"), "got: {out}");
    }

    #[test]
    fn ignore_blank_drops_empty_items() {
        let out = d("a\n\n\nb", "b");
        assert!(out.contains("Totals: A=2 · B=1"), "got: {out}");
    }

    #[test]
    fn rejects_invalid_separator() {
        let err = diff("a", "b", "colon", false, true, true, true, false, "input").unwrap_err();
        assert!(err.contains("invalid separator"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_sort() {
        let err =
            diff("a", "b", "newline", false, true, true, true, false, "random").unwrap_err();
        assert!(err.contains("invalid sort"), "got: {err}");
    }
}
