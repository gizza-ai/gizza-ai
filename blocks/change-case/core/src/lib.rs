//! change-case core — convert text between letter cases. No wafer/wasm-bindgen
//! deps; shared by the chat skill block and the web page.
//!
//! Two families of conversions:
//!
//! * **Letter-case transforms** keep the original spacing/punctuation and only
//!   re-map letters: `upper`, `lower`, `title`, `sentence`, `swap`, `capitalize`.
//! * **Identifier (programmer) cases** re-tokenize the text into words and join
//!   them with a delimiter: `camel`, `pascal`, `snake`, `kebab`, `constant`,
//!   `dot`, `path`, `train`. These collapse runs of spaces/symbols into single
//!   word boundaries (e.g. `"hello world-foo"` -> `helloWorldFoo`).

/// Convert `text` to the requested `mode` (case-insensitive; blank → `upper`).
///
/// Supported modes:
/// - `upper` — UPPERCASE everything.
/// - `lower` — lowercase everything.
/// - `title` — Capitalize The First Letter Of Each Word.
/// - `sentence` — Capitalize the first letter of each sentence (`. ! ?` end one).
/// - `capitalize` — Capitalize only the very first letter; lowercase the rest.
/// - `swap` — sWAP tHE cASE of every letter.
/// - `alternate` — aLtErNaTe lower/upper across the letters.
/// - `camel` — camelCase identifier.
/// - `pascal` — PascalCase identifier.
/// - `snake` — snake_case identifier.
/// - `constant` — CONSTANT_CASE identifier (UPPER snake).
/// - `kebab` — kebab-case identifier.
/// - `train` — Train-Case identifier (Capitalized, hyphen-joined).
/// - `dot` — dot.case identifier.
/// - `path` — path/case identifier (slash-joined).
///
/// Returns `Err` for an unknown mode.
pub fn convert(text: &str, mode: &str) -> Result<String, String> {
    let m = mode.trim();
    let m = if m.is_empty() { "upper" } else { m };
    match m.to_ascii_lowercase().as_str() {
        "upper" | "uppercase" => Ok(text.to_uppercase()),
        "lower" | "lowercase" => Ok(text.to_lowercase()),
        "title" => Ok(title_case(text)),
        "sentence" => Ok(sentence_case(text)),
        "capitalize" => Ok(capitalize(text)),
        "swap" | "toggle" | "invert" => Ok(swap_case(text)),
        "alternate" | "alternating" | "spongebob" => Ok(alternate_case(text)),
        "camel" => Ok(join_words(text, Join::Camel)),
        "pascal" => Ok(join_words(text, Join::Pascal)),
        "snake" => Ok(join_words(text, Join::Snake)),
        "constant" | "macro" | "screaming" => Ok(join_words(text, Join::Constant)),
        "kebab" => Ok(join_words(text, Join::Kebab)),
        "train" => Ok(join_words(text, Join::Train)),
        "dot" => Ok(join_words(text, Join::Dot)),
        "path" => Ok(join_words(text, Join::Path)),
        other => Err(format!(
            "invalid mode {other:?}: expected one of upper, lower, title, \
sentence, capitalize, swap, alternate, camel, pascal, snake, constant, kebab, train, dot, path"
        )),
    }
}

/// Capitalize the first letter of every whitespace-separated word; the rest of
/// each word is lowercased (the common "Title Case" of a heading).
fn title_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_word_start = true;
    for c in text.chars() {
        if c.is_whitespace() {
            at_word_start = true;
            out.push(c);
        } else if at_word_start {
            extend_upper(&mut out, c);
            at_word_start = false;
        } else {
            extend_lower(&mut out, c);
        }
    }
    out
}

/// Capitalize the first letter of each sentence (a sentence ends at `.`, `!`, or
/// `?`); every other letter is lowercased.
fn sentence_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut start_of_sentence = true;
    for c in text.chars() {
        if start_of_sentence && c.is_alphabetic() {
            extend_upper(&mut out, c);
            start_of_sentence = false;
        } else {
            extend_lower(&mut out, c);
            if matches!(c, '.' | '!' | '?') {
                start_of_sentence = true;
            }
        }
    }
    out
}

/// Capitalize only the first letter of the whole string; lowercase the rest.
fn capitalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut first = true;
    for c in text.chars() {
        if first && c.is_alphabetic() {
            extend_upper(&mut out, c);
            first = false;
        } else {
            extend_lower(&mut out, c);
        }
    }
    out
}

/// Alternate lower/upper across the letters, starting lowercase. Non-letters
/// pass through without advancing the alternation, so `"hi there"` becomes
/// `"hI ThErE"`.
fn alternate_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut upper = false;
    for c in text.chars() {
        if c.is_alphabetic() {
            if upper {
                extend_upper(&mut out, c);
            } else {
                extend_lower(&mut out, c);
            }
            upper = !upper;
        } else {
            out.push(c);
        }
    }
    out
}

/// Invert the case of every letter (lowercase ↔ uppercase).
fn swap_case(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_uppercase() {
            extend_lower(&mut out, c);
        } else if c.is_lowercase() {
            extend_upper(&mut out, c);
        } else {
            out.push(c);
        }
    }
    out
}

fn extend_upper(out: &mut String, c: char) {
    for u in c.to_uppercase() {
        out.push(u);
    }
}

fn extend_lower(out: &mut String, c: char) {
    for l in c.to_lowercase() {
        out.push(l);
    }
}

/// How to join the tokenized words for an identifier case.
#[derive(Clone, Copy)]
enum Join {
    Camel,
    Pascal,
    Snake,
    Constant,
    Kebab,
    Train,
    Dot,
    Path,
}

/// Split `text` into words, then join them in the requested identifier style.
///
/// Word boundaries are: any non-alphanumeric run (spaces, `_`, `-`, `.`, `/`,
/// punctuation), AND a lower→upper transition inside a run of letters
/// (`"helloWorld"` → `["hello", "world"]`), AND a letter↔digit transition
/// (`"v2Engine"` → `["v", "2", "engine"]`). Acronyms are kept whole then split
/// before the last cap of a run (`"HTTPServer"` → `["http", "server"]`).
fn join_words(text: &str, join: Join) -> String {
    let words = tokenize(text);
    let pieces: Vec<String> = words
        .iter()
        .enumerate()
        .map(|(i, w)| match join {
            Join::Camel => {
                if i == 0 {
                    w.to_lowercase()
                } else {
                    cap_first(w)
                }
            }
            Join::Pascal => cap_first(w),
            Join::Snake | Join::Kebab | Join::Dot | Join::Path => w.to_lowercase(),
            Join::Constant => w.to_uppercase(),
            Join::Train => cap_first(w),
        })
        .collect();
    let sep = match join {
        Join::Camel | Join::Pascal => "",
        Join::Snake | Join::Constant => "_",
        Join::Kebab | Join::Train => "-",
        Join::Dot => ".",
        Join::Path => "/",
    };
    pieces.join(sep)
}

/// Lowercase a word and capitalize its first letter.
fn cap_first(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out = String::new();
            extend_upper(&mut out, first);
            for c in chars {
                extend_lower(&mut out, c);
            }
            out
        }
    }
}

/// Break text into normalized words (see `join_words` for the rules).
fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();

    let flush = |cur: &mut String, words: &mut Vec<String>| {
        if !cur.is_empty() {
            words.push(std::mem::take(cur));
        }
    };

    for i in 0..chars.len() {
        let c = chars[i];
        if !c.is_alphanumeric() {
            // delimiter — ends the current word
            flush(&mut cur, &mut words);
            continue;
        }
        if !cur.is_empty() {
            let prev = chars[i - 1];
            let lower_to_upper = prev.is_lowercase() && c.is_uppercase();
            let letter_digit = prev.is_alphabetic() != c.is_alphabetic()
                && (prev.is_numeric() || c.is_numeric());
            // ACRONYM boundary: in a run like "HTTPServer", split before the
            // last uppercase that is followed by a lowercase (the 'S' here).
            let acronym = prev.is_uppercase()
                && c.is_uppercase()
                && i + 1 < chars.len()
                && chars[i + 1].is_lowercase();
            if lower_to_upper || letter_digit || acronym {
                flush(&mut cur, &mut words);
            }
        }
        cur.push(c);
    }
    flush(&mut cur, &mut words);
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upper_lower() {
        assert_eq!(convert("Hello World", "upper").unwrap(), "HELLO WORLD");
        assert_eq!(convert("Hello World", "lower").unwrap(), "hello world");
        // blank mode defaults to upper.
        assert_eq!(convert("hi", "").unwrap(), "HI");
        // mode is case-insensitive.
        assert_eq!(convert("hi", "UPPER").unwrap(), "HI");
    }

    #[test]
    fn title_capitalizes_each_word() {
        assert_eq!(
            convert("the quick brown FOX", "title").unwrap(),
            "The Quick Brown Fox"
        );
        // whitespace is preserved exactly.
        assert_eq!(convert("a  b", "title").unwrap(), "A  B");
    }

    #[test]
    fn sentence_capitalizes_each_sentence() {
        assert_eq!(
            convert("hello world. how ARE you? fine!", "sentence").unwrap(),
            "Hello world. How are you? Fine!"
        );
    }

    #[test]
    fn capitalize_only_first() {
        assert_eq!(
            convert("hELLO wORLD", "capitalize").unwrap(),
            "Hello world"
        );
    }

    #[test]
    fn swap_inverts_case() {
        assert_eq!(convert("Hello World", "swap").unwrap(), "hELLO wORLD");
    }

    #[test]
    fn alternate_case_alternates_letters_only() {
        assert_eq!(convert("hi there", "alternate").unwrap(), "hI tHeRe");
    }

    #[test]
    fn identifier_cases() {
        let s = "Hello world-foo";
        assert_eq!(convert(s, "camel").unwrap(), "helloWorldFoo");
        assert_eq!(convert(s, "pascal").unwrap(), "HelloWorldFoo");
        assert_eq!(convert(s, "snake").unwrap(), "hello_world_foo");
        assert_eq!(convert(s, "constant").unwrap(), "HELLO_WORLD_FOO");
        assert_eq!(convert(s, "kebab").unwrap(), "hello-world-foo");
        assert_eq!(convert(s, "train").unwrap(), "Hello-World-Foo");
        assert_eq!(convert(s, "dot").unwrap(), "hello.world.foo");
        assert_eq!(convert(s, "path").unwrap(), "hello/world/foo");
    }

    #[test]
    fn tokenizes_existing_camel_and_acronyms() {
        assert_eq!(convert("helloWorld", "snake").unwrap(), "hello_world");
        assert_eq!(convert("HTTPServer", "snake").unwrap(), "http_server");
        assert_eq!(convert("getURLFromAPI", "kebab").unwrap(), "get-url-from-api");
        assert_eq!(convert("v2Engine", "snake").unwrap(), "v_2_engine");
    }

    #[test]
    fn unicode_case_mapping() {
        assert_eq!(convert("straße", "upper").unwrap(), "STRASSE");
        assert_eq!(convert("CAFÉ", "lower").unwrap(), "café");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = convert("x", "wingdings").unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn empty_input_is_ok() {
        assert_eq!(convert("", "snake").unwrap(), "");
        assert_eq!(convert("", "upper").unwrap(), "");
    }
}
