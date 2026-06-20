//! gizza-ai/find-replace core — pure find & replace over text. No
//! wafer/wasm-bindgen deps. Supports literal or regular-expression matching,
//! case sensitivity, and replace-all vs replace-first. Uses the `regex` crate;
//! literal mode escapes the needle and treats the replacement literally (so `$`
//! is not a group reference), while regex mode supports `$1`/`$name` references.

use regex::{NoExpand, RegexBuilder};

/// Find/replace `find` in `text`. Returns the new text and the number of
/// matches replaced.
pub fn find_replace(
    text: &str,
    find: &str,
    replacement: &str,
    is_regex: bool,
    case_sensitive: bool,
    global: bool,
) -> Result<(String, usize), String> {
    if find.is_empty() {
        return Err("`find` must not be empty".into());
    }
    let pattern = if is_regex {
        find.to_string()
    } else {
        regex::escape(find)
    };
    let re = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))?;

    let count = if global {
        re.find_iter(text).count()
    } else {
        re.find(text).map(|_| 1).unwrap_or(0)
    };

    let out = match (is_regex, global) {
        // Regex mode: replacement may use $1 / ${name} capture references.
        (true, true) => re.replace_all(text, replacement).into_owned(),
        (true, false) => re.replace(text, replacement).into_owned(),
        // Literal mode: replacement is taken verbatim (NoExpand → `$` is literal).
        (false, true) => re.replace_all(text, NoExpand(replacement)).into_owned(),
        (false, false) => re.replace(text, NoExpand(replacement)).into_owned(),
    };
    Ok((out, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_global_escapes_metachars() {
        // The '.' is literal, so only the two real dots match.
        let (out, n) = find_replace("a.b.c", ".", "-", false, true, true).unwrap();
        assert_eq!(out, "a-b-c");
        assert_eq!(n, 2);
    }

    #[test]
    fn regex_digit_class() {
        let (out, n) = find_replace("a1b2c3", r"\d", "#", true, true, true).unwrap();
        assert_eq!(out, "a#b#c#");
        assert_eq!(n, 3);
    }

    #[test]
    fn case_insensitive() {
        let (out, n) = find_replace("Hello hello HELLO", "hello", "hi", false, false, true).unwrap();
        assert_eq!(out, "hi hi hi");
        assert_eq!(n, 3);
    }

    #[test]
    fn case_sensitive_default() {
        let (out, n) = find_replace("Hello hello", "hello", "hi", false, true, true).unwrap();
        assert_eq!(out, "Hello hi");
        assert_eq!(n, 1);
    }

    #[test]
    fn replace_first_only() {
        let (out, n) = find_replace("aaa", "a", "b", false, true, false).unwrap();
        assert_eq!(out, "baa");
        assert_eq!(n, 1);
    }

    #[test]
    fn regex_capture_group_refs() {
        let (out, n) =
            find_replace("John Smith", r"(\w+)\s+(\w+)", "$2 $1", true, true, true).unwrap();
        assert_eq!(out, "Smith John");
        assert_eq!(n, 1);
    }

    #[test]
    fn literal_dollar_is_not_expanded() {
        // In literal mode the replacement "$5" stays verbatim (no group expansion).
        let (out, n) = find_replace("price", "price", "$5", false, true, true).unwrap();
        assert_eq!(out, "$5");
        assert_eq!(n, 1);
    }

    #[test]
    fn no_match_is_zero_and_unchanged() {
        let (out, n) = find_replace("abc", "z", "Y", false, true, true).unwrap();
        assert_eq!(out, "abc");
        assert_eq!(n, 0);
    }

    #[test]
    fn errors() {
        assert!(find_replace("x", "", "y", false, true, true).is_err()); // empty find
        assert!(find_replace("x", "(", "y", true, true, true).is_err()); // invalid regex
    }
}
