//! list-dedupe-merge core — merge two lists into one deduplicated list and
//! report how many overlapping entries were collapsed. Pure compute, no
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

use std::collections::HashSet;

/// Maximum items processed per list. A generous cap that keeps the pure,
/// single-threaded merge instant while guarding against pathological input.
pub const MAX_ITEMS: usize = 100_000;

/// How the two lists are combined before de-duplication.
#[derive(Clone, Copy)]
enum Order {
    /// All of list A (in order), then all of list B.
    Append,
    /// Alternate A[0], B[0], A[1], B[1], … (the longer list's tail follows).
    Interleave,
}

impl Order {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "append" => Ok(Order::Append),
            "interleave" => Ok(Order::Interleave),
            other => Err(format!(
                "invalid merge_order {other:?}: expected \"append\" or \"interleave\""
            )),
        }
    }
}

/// Output ordering for the merged list.
#[derive(Clone, Copy)]
enum Sort {
    /// Keep the order items first appear in the merged sequence.
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

/// Interleave two item lists: A[0], B[0], A[1], B[1], … then the longer list's
/// leftover tail, preserving each list's relative order.
fn interleave(a: Vec<Item>, b: Vec<Item>) -> Vec<Item> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let mut ai = a.into_iter();
    let mut bi = b.into_iter();
    loop {
        match (ai.next(), bi.next()) {
            (Some(x), Some(y)) => {
                out.push(x);
                out.push(y);
            }
            (Some(x), None) => {
                out.push(x);
                out.extend(ai);
                break;
            }
            (None, Some(y)) => {
                out.push(y);
                out.extend(bi);
                break;
            }
            (None, None) => break,
        }
    }
    out
}

/// Render the merged section: a header with the item count, then the items one
/// per line (or `(none)` when empty).
fn render_section(items: &[String]) -> String {
    let body = if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join("\n")
    };
    format!("Merged ({}):\n{body}", items.len())
}

/// Merge `list_a` and `list_b` into one de-duplicated list and return a
/// human-readable report: the merged list followed by a totals line that reports
/// how many duplicates were collapsed and how many entries were shared by both
/// lists (the cross-list overlap).
///
/// - `separator` (`newline` default | `comma` | `tab` | `semicolon` | `pipe` |
///   `space`): how each list's items are split.
/// - `merge_order` (`append` default | `interleave`): how the two lists are
///   combined before de-duplication. First occurrence wins on a tie, so the
///   kept display form comes from whichever list contributed it first.
/// - `trim`: strip leading/trailing whitespace from each item (default true).
/// - `ignore_blank`: drop empty items (default true).
/// - `ignore_case`: match case-insensitively (Apple == apple, default false).
/// - `sort` (`input` default | `asc` | `desc`): ordering of the merged list.
/// - `ignore_leading_zeros`: treat e.g. `007` and `7` as equal (default false).
///
/// De-duplication is always on — that is the point of this tool; the output is
/// the set union of the two lists. `merged = A + B − duplicates removed`, and
/// `shared by both` counts the distinct entries that appeared in both lists.
#[allow(clippy::too_many_arguments)]
pub fn merge(
    list_a: &str,
    list_b: &str,
    separator: &str,
    merge_order: &str,
    trim: bool,
    ignore_blank: bool,
    ignore_case: bool,
    sort: &str,
    ignore_leading_zeros: bool,
) -> Result<String, String> {
    let sep = separator_char(separator)?;
    let order = Order::parse(merge_order)?;
    let sort = Sort::parse(sort)?;

    let a = process(
        list_a, sep, "A", trim, ignore_blank, ignore_case, ignore_leading_zeros,
    )?;
    let b = process(
        list_b, sep, "B", trim, ignore_blank, ignore_case, ignore_leading_zeros,
    )?;

    let a_count = a.len();
    let b_count = b.len();

    // Cross-list overlap (distinct keys present in BOTH lists) — the headline
    // "overlapping entries collapsed" number.
    let a_keys: HashSet<&str> = a.iter().map(|it| it.key.as_str()).collect();
    let b_keys: HashSet<&str> = b.iter().map(|it| it.key.as_str()).collect();
    let shared_by_both = a_keys.intersection(&b_keys).count();

    // Combine per merge_order, then de-duplicate first-seen (union semantics).
    let combined = match order {
        Order::Append => {
            let mut v = a;
            v.extend(b);
            v
        }
        Order::Interleave => interleave(a, b),
    };

    let mut seen: HashSet<String> = HashSet::new();
    let mut merged: Vec<String> = Vec::with_capacity(combined.len());
    for it in combined {
        if seen.insert(it.key) {
            merged.push(it.display);
        }
    }
    let merged_count = merged.len();
    let duplicates_removed = a_count + b_count - merged_count;

    match sort {
        Sort::Input => {}
        Sort::Asc => merged.sort(),
        Sort::Desc => {
            merged.sort();
            merged.reverse();
        }
    }

    Ok(format!(
        "{}\n\nTotals: A={} · B={} · merged={} · duplicates removed={} · shared by both={}",
        render_section(&merged),
        a_count,
        b_count,
        merged_count,
        duplicates_removed,
        shared_by_both,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convenience wrapper with the tool's defaults.
    fn m(a: &str, b: &str) -> String {
        merge(a, b, "newline", "append", true, true, false, "input", false).unwrap()
    }

    #[test]
    fn basic_append_merge_with_counts() {
        let out = m("apple\nbanana\ncherry", "banana\ncherry\ndate");
        assert_eq!(
            out,
            "Merged (4):\napple\nbanana\ncherry\ndate\n\n\
             Totals: A=3 · B=3 · merged=4 · duplicates removed=2 · shared by both=2"
        );
    }

    #[test]
    fn interleave_orders_alternately() {
        let out = merge(
            "1\n2\n3", "a\nb\nc", "newline", "interleave", true, true, false, "input", false,
        )
        .unwrap();
        // 1, a, 2, b, 3, c — no duplicates, alternating order preserved.
        assert!(out.contains("Merged (6):\n1\na\n2\nb\n3\nc"), "got: {out}");
        assert!(
            out.contains("Totals: A=3 · B=3 · merged=6 · duplicates removed=0 · shared by both=0"),
            "got: {out}"
        );
    }

    #[test]
    fn append_and_interleave_differ_in_order() {
        let appended = merge(
            "1\n2\n3", "a\nb\nc", "newline", "append", true, true, false, "input", false,
        )
        .unwrap();
        assert!(appended.contains("Merged (6):\n1\n2\n3\na\nb\nc"), "got: {appended}");
    }

    #[test]
    fn ignore_case_matches_across_case_and_keeps_first_form() {
        let out = merge(
            "Alice@x.com\nBob@x.com",
            "bob@x.com\ncarol@x.com",
            "newline",
            "append",
            true,
            true,
            true,
            "input",
            false,
        )
        .unwrap();
        // bob@x.com is shared (case-insensitive); display keeps A's form "Bob@x.com".
        assert_eq!(
            out,
            "Merged (3):\nAlice@x.com\nBob@x.com\ncarol@x.com\n\n\
             Totals: A=2 · B=2 · merged=3 · duplicates removed=1 · shared by both=1"
        );
    }

    #[test]
    fn comma_separator_and_asc_sort() {
        let out = merge(
            "delta,alpha,charlie", "alpha,bravo", "comma", "append", true, true, false, "asc",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "Merged (4):\nalpha\nbravo\ncharlie\ndelta\n\n\
             Totals: A=3 · B=2 · merged=4 · duplicates removed=1 · shared by both=1"
        );
    }

    #[test]
    fn desc_sort_reverses() {
        let out = merge(
            "b\na", "c", "newline", "append", true, true, false, "desc", false,
        )
        .unwrap();
        assert!(out.contains("Merged (3):\nc\nb\na"), "got: {out}");
    }

    #[test]
    fn ignore_leading_zeros_matches_numeric_ids() {
        let out = merge(
            "007\n042\n100", "7\n100\n250", "newline", "append", true, true, false, "input", true,
        )
        .unwrap();
        // 007 == 7 and 100 == 100 → collapsed; 250 is new.
        assert_eq!(
            out,
            "Merged (4):\n007\n042\n100\n250\n\n\
             Totals: A=3 · B=3 · merged=4 · duplicates removed=2 · shared by both=2"
        );
    }

    #[test]
    fn within_list_duplicates_are_collapsed_too() {
        // A repeats "x"; duplicates_removed counts the within-list repeat plus
        // the cross-list overlap, but shared_by_both only counts cross-list.
        let out = m("x\nx\ny", "y\nz");
        assert!(out.contains("Merged (3):\nx\ny\nz"), "got: {out}");
        assert!(
            out.contains("Totals: A=3 · B=2 · merged=3 · duplicates removed=2 · shared by both=1"),
            "got: {out}"
        );
    }

    #[test]
    fn ignore_blank_drops_empty_items() {
        let out = m("a\n\n\nb", "b");
        assert!(out.contains("Totals: A=2 · B=1"), "got: {out}");
        assert!(out.contains("Merged (2):\na\nb"), "got: {out}");
    }

    #[test]
    fn empty_lists_show_none() {
        let out = m("", "");
        assert!(out.contains("Merged (0):\n(none)"), "got: {out}");
        assert!(
            out.contains("Totals: A=0 · B=0 · merged=0 · duplicates removed=0 · shared by both=0"),
            "got: {out}"
        );
    }

    #[test]
    fn rejects_invalid_separator() {
        let err = merge("a", "b", "colon", "append", true, true, false, "input", false)
            .unwrap_err();
        assert!(err.contains("invalid separator"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_merge_order() {
        let err = merge("a", "b", "newline", "zip", true, true, false, "input", false)
            .unwrap_err();
        assert!(err.contains("invalid merge_order"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_sort() {
        let err = merge("a", "b", "newline", "append", true, true, false, "random", false)
            .unwrap_err();
        assert!(err.contains("invalid sort"), "got: {err}");
    }
}
