//! accent-stripper core — strip diacritics and transliterate text to plain ASCII.
//! Pure compute, shared by the chat skill block, the CLI and the web page.
//!
//! Two conversion modes:
//!   * [`Mode::Transliterate`] (default) — every non-ASCII character is replaced
//!     by its closest plain-ASCII spelling via the `deunicode` table, so beyond
//!     plain accents it also handles letters that carry no combining mark at all
//!     (`ß` → `ss`, `ø` → `o`, `æ` → `ae`, `đ` → `d`, `ł` → `l`) plus whole
//!     non-Latin scripts (`Живpost` → `Zhivpost`, `北京` → `Bei Jing`).
//!   * [`Mode::MarksOnly`] — the conservative classic: decompose (NFD), drop the
//!     combining marks, keep everything else exactly as typed. `café` → `cafe`
//!     but `Straße` and `Ж` are left alone.
//!
//! Whatever the mode leaves behind that is still not ASCII is then handled by the
//! [`Unmapped`] policy (keep it / remove it / replace it), so "give me pure
//! ASCII, no exceptions" is expressible in both modes.
//!
//! Pipeline order: convert → `unmapped` policy → lowercase → collapse whitespace.

use deunicode::{deunicode, deunicode_char};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Maximum number of characters accepted in one run. Keeps a paste-happy user
/// inside the wasm sandbox's memory budget with a clear message instead of a trap.
pub const MAX_INPUT_CHARS: usize = 200_000;

/// Longest `replacement` string accepted for [`Unmapped::Replace`].
pub const MAX_REPLACEMENT_CHARS: usize = 8;

/// How accented / non-ASCII letters are converted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Full ASCII transliteration (`ß` → `ss`, `Ж` → `Zh`, `北` → `Bei`).
    Transliterate,
    /// Decompose and drop combining marks only (`é` → `e`, `ß` untouched).
    MarksOnly,
}

impl Mode {
    /// Parse the wire value. Blank means the default (`transliterate`).
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "transliterate" => Ok(Mode::Transliterate),
            "marks-only" | "marks_only" | "marks" => Ok(Mode::MarksOnly),
            other => Err(format!(
                "unknown mode '{other}': expected 'transliterate' or 'marks-only'"
            )),
        }
    }
}

/// What to do with characters that have no ASCII form after conversion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unmapped {
    /// Leave the character in the output as-is (default).
    Keep,
    /// Delete the character.
    Remove,
    /// Swap the character for the `replacement` string.
    Replace,
}

impl Unmapped {
    /// Parse the wire value. Blank means the default (`keep`).
    pub fn parse(s: &str) -> Result<Unmapped, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" => Ok(Unmapped::Keep),
            "remove" | "drop" => Ok(Unmapped::Remove),
            "replace" => Ok(Unmapped::Replace),
            other => Err(format!(
                "unknown unmapped policy '{other}': expected 'keep', 'remove' or 'replace'"
            )),
        }
    }
}

/// Every knob the conversion takes. Build with [`Options::default`] then override.
#[derive(Clone, Debug)]
pub struct Options {
    /// Conversion strategy.
    pub mode: Mode,
    /// Policy for characters with no ASCII form.
    pub unmapped: Unmapped,
    /// Text substituted for each unmapped character when `unmapped` is `Replace`.
    pub replacement: String,
    /// Characters passed through untouched, whatever the mode says.
    pub keep: String,
    /// Lowercase the converted text.
    pub lowercase: bool,
    /// Collapse runs of spaces/tabs inside each line and trim the line ends.
    pub collapse_whitespace: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Transliterate,
            unmapped: Unmapped::Keep,
            replacement: "?".to_string(),
            keep: String::new(),
            lowercase: false,
            collapse_whitespace: false,
        }
    }
}

/// The converted text plus an audit of what happened to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// The converted text.
    pub result: String,
    /// Characters in the input.
    pub input_chars: usize,
    /// Characters in the output.
    pub output_chars: usize,
    /// Non-ASCII characters in the input.
    pub non_ascii_in: usize,
    /// Non-ASCII input characters that were converted to ASCII.
    pub converted: usize,
    /// Non-ASCII input characters preserved by the `keep` list.
    pub kept: usize,
    /// Non-ASCII input characters that hit the `unmapped` policy.
    pub unmapped: usize,
    /// Up to 20 distinct characters that hit the `unmapped` policy, in first-seen order.
    pub unmapped_samples: Vec<String>,
    /// True when every character of the output is ASCII.
    pub output_is_ascii: bool,
}

/// Longest list of distinct unmapped characters reported back.
const MAX_UNMAPPED_SAMPLES: usize = 20;

/// Strip accents from `text` per `opts`.
///
/// # Errors
/// Returns a human-readable message when the input is longer than
/// [`MAX_INPUT_CHARS`], or when `replacement` is not ASCII / is longer than
/// [`MAX_REPLACEMENT_CHARS`] characters.
pub fn strip(text: &str, opts: &Options) -> Result<Report, String> {
    let input_chars = text.chars().count();
    if input_chars > MAX_INPUT_CHARS {
        return Err(format!(
            "input is {input_chars} characters; the limit is {MAX_INPUT_CHARS} — split the text and run it in parts"
        ));
    }
    if opts.unmapped == Unmapped::Replace {
        if !opts.replacement.is_ascii() {
            return Err(format!(
                "replacement '{}' must be plain ASCII — it is what stands in for characters that have no ASCII form",
                opts.replacement
            ));
        }
        if opts.replacement.chars().count() > MAX_REPLACEMENT_CHARS {
            return Err(format!(
                "replacement is {} characters; the limit is {MAX_REPLACEMENT_CHARS}",
                opts.replacement.chars().count()
            ));
        }
    }

    let keep: Vec<char> = opts.keep.chars().filter(|c| !c.is_ascii()).collect();
    let mut st = State {
        out: String::with_capacity(text.len()),
        pending: String::new(),
        non_ascii_in: 0,
        converted: 0,
        kept: 0,
        unmapped: 0,
        samples: Vec::new(),
    };

    for ch in text.chars() {
        if ch.is_ascii() {
            st.flush();
            st.out.push(ch);
            continue;
        }
        st.non_ascii_in += 1;
        if keep.contains(&ch) {
            st.flush();
            st.out.push(ch);
            st.kept += 1;
            continue;
        }
        match opts.mode {
            Mode::Transliterate => match deunicode_char(ch) {
                // Buffer runs of transliterable characters so `deunicode` itself
                // owns the word spacing it inserts between CJK syllables.
                Some(_) => {
                    st.pending.push(ch);
                    st.converted += 1;
                }
                None => {
                    st.flush();
                    st.apply_unmapped(ch, opts);
                }
            },
            Mode::MarksOnly => {
                let stripped: String = ch.nfd().filter(|c| !is_combining_mark(*c)).collect();
                if stripped.is_empty() {
                    // A lone combining mark: dropping it IS the conversion.
                    st.converted += 1;
                } else if stripped.is_ascii() {
                    st.out.push_str(&stripped);
                    st.converted += 1;
                } else {
                    // Still non-ASCII with its marks gone (ß, ø, Ж, 北) — the
                    // policy decides, and `keep` keeps the mark-stripped base.
                    st.apply_unmapped_str(ch, &stripped, opts);
                }
            }
        }
    }
    st.flush();

    let mut result = st.out;
    if opts.lowercase {
        result = result.to_lowercase();
    }
    if opts.collapse_whitespace {
        result = collapse_whitespace(&result);
    }

    Ok(Report {
        output_chars: result.chars().count(),
        output_is_ascii: result.is_ascii(),
        result,
        input_chars,
        non_ascii_in: st.non_ascii_in,
        converted: st.converted,
        kept: st.kept,
        unmapped: st.unmapped,
        unmapped_samples: st.samples,
    })
}

/// Accumulator for [`strip`]: the finished output plus the run of characters
/// still waiting to be transliterated as one `deunicode` segment.
struct State {
    out: String,
    pending: String,
    non_ascii_in: usize,
    converted: usize,
    kept: usize,
    unmapped: usize,
    samples: Vec<String>,
}

impl State {
    /// Transliterate and emit the buffered run, if any.
    fn flush(&mut self) {
        if !self.pending.is_empty() {
            self.out.push_str(&deunicode(&self.pending));
            self.pending.clear();
        }
    }

    /// Apply the unmapped policy to a character that stayed non-ASCII.
    fn apply_unmapped(&mut self, ch: char, opts: &Options) {
        let as_str = ch.to_string();
        self.apply_unmapped_str(ch, &as_str, opts);
    }

    /// As [`State::apply_unmapped`], but `keep` emits `text` (the mark-stripped
    /// form) rather than the original character.
    fn apply_unmapped_str(&mut self, ch: char, text: &str, opts: &Options) {
        self.unmapped += 1;
        let sample = ch.to_string();
        if self.samples.len() < MAX_UNMAPPED_SAMPLES && !self.samples.contains(&sample) {
            self.samples.push(sample);
        }
        match opts.unmapped {
            Unmapped::Keep => self.out.push_str(text),
            Unmapped::Remove => {}
            Unmapped::Replace => self.out.push_str(&opts.replacement),
        }
    }
}

/// Squeeze runs of spaces/tabs inside each line to one space and trim the line
/// ends. Line breaks survive so pasted documents keep their shape.
fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut last_was_space = false;
        for ch in line.trim().chars() {
            let is_space = ch.is_whitespace();
            if is_space {
                if !last_was_space {
                    out.push(' ');
                }
            } else {
                out.push(ch);
            }
            last_was_space = is_space;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, opts: &Options) -> String {
        strip(text, opts).expect("conversion should succeed").result
    }

    #[test]
    fn transliterates_accented_latin_to_ascii() {
        let out = run("Crème Brûlée à la Française", &Options::default());
        assert_eq!(out, "Creme Brulee a la Francaise");
    }

    #[test]
    fn transliterate_handles_letters_without_combining_marks() {
        // The whole point of the transliterate mode: these carry no combining
        // mark, so NFD-and-drop can never fix them.
        let opts = Options::default();
        assert_eq!(run("Straße", &opts), "Strasse");
        assert_eq!(run("Ærøskøbing", &opts), "AEroskobing");
        assert_eq!(run("Łódź", &opts), "Lodz");
        assert_eq!(run("Đà Nẵng", &opts), "Da Nang");
    }

    #[test]
    fn transliterate_covers_non_latin_scripts() {
        let opts = Options::default();
        assert_eq!(run("Москва", &opts), "Moskva");
        assert_eq!(run("Ελλάδα", &opts), "Ellada");
        // CJK gets the crate's own syllable spacing, not a run-on.
        assert_eq!(run("北京", &opts), "Bei Jing");
    }

    #[test]
    fn marks_only_leaves_unmarked_letters_alone() {
        let opts = Options {
            mode: Mode::MarksOnly,
            ..Options::default()
        };
        assert_eq!(run("Crème Brûlée", &opts), "Creme Brulee");
        // No combining mark to drop → untouched, unlike transliterate mode.
        assert_eq!(run("Straße", &opts), "Straße");
        assert_eq!(run("Ærøskøbing", &opts), "Ærøskøbing");
    }

    #[test]
    fn marks_only_handles_already_decomposed_input() {
        // "cafe" + U+0301 COMBINING ACUTE ACCENT — no precomposed é anywhere.
        let opts = Options {
            mode: Mode::MarksOnly,
            ..Options::default()
        };
        assert_eq!(run("cafe\u{0301}", &opts), "cafe");
    }

    #[test]
    fn ascii_input_is_returned_untouched() {
        let r = strip("Plain ASCII, 123!", &Options::default()).unwrap();
        assert_eq!(r.result, "Plain ASCII, 123!");
        assert_eq!(r.non_ascii_in, 0);
        assert_eq!(r.converted, 0);
        assert!(r.output_is_ascii);
    }

    #[test]
    fn punctuation_spacing_and_case_survive_by_default() {
        let out = run(
            "  Señor  O'Neill —  ¿Qué tal?\nSegunda línea  ",
            &Options::default(),
        );
        assert_eq!(out, "  Senor  O'Neill --  ?Que tal?\nSegunda linea  ");
    }

    #[test]
    fn unmapped_policies_apply_to_leftovers() {
        // U+1D54F MATHEMATICAL DOUBLE-STRUCK CAPITAL X has no marks-only fix.
        let base = Options {
            mode: Mode::MarksOnly,
            ..Options::default()
        };
        assert_eq!(run("a𝕏ø", &base), "a𝕏ø");
        assert_eq!(
            run(
                "a𝕏ø",
                &Options {
                    unmapped: Unmapped::Remove,
                    ..base.clone()
                }
            ),
            "a"
        );
        let replaced = strip(
            "a𝕏ø",
            &Options {
                unmapped: Unmapped::Replace,
                replacement: "_".into(),
                ..base
            },
        )
        .unwrap();
        assert_eq!(replaced.result, "a__");
        assert_eq!(replaced.unmapped, 2);
        assert_eq!(replaced.unmapped_samples, vec!["𝕏", "ø"]);
        assert!(replaced.output_is_ascii);
    }

    #[test]
    fn keep_list_protects_chosen_characters() {
        let opts = Options {
            keep: "ñ".into(),
            ..Options::default()
        };
        let r = strip("mañana café", &opts).unwrap();
        assert_eq!(r.result, "mañana cafe");
        assert_eq!(r.kept, 1);
        assert_eq!(r.converted, 1);
        assert!(!r.output_is_ascii);
    }

    #[test]
    fn lowercase_and_collapse_whitespace_run_last() {
        let opts = Options {
            lowercase: true,
            collapse_whitespace: true,
            ..Options::default()
        };
        assert_eq!(
            run("  ÉCOLE   Normale \n  Supérieure  ", &opts),
            "ecole normale\nsuperieure"
        );
    }

    #[test]
    fn transliterate_reaches_styled_letters_marks_only_cannot() {
        // U+1D54F MATHEMATICAL DOUBLE-STRUCK CAPITAL X carries no combining
        // mark, so only the transliterate table gets it back to a plain letter.
        assert_eq!(run("𝕏", &Options::default()), "X");
    }

    #[test]
    fn report_counts_every_class_of_character() {
        // U+E000 is a private-use code point — nothing can transliterate it.
        let r = strip("café 北 \u{E000}", &Options::default()).unwrap();
        assert_eq!(r.input_chars, 8);
        assert_eq!(r.non_ascii_in, 3);
        assert_eq!(r.converted, 2);
        assert_eq!(r.kept, 0);
        assert_eq!(r.unmapped, 1);
        assert_eq!(r.unmapped_samples, vec!["\u{E000}"]);
        assert_eq!(r.result, "cafe Bei \u{E000}");
        assert_eq!(r.output_chars, 10);
        assert!(!r.output_is_ascii);
    }

    #[test]
    fn mode_and_policy_parse_their_wire_values() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Transliterate);
        assert_eq!(Mode::parse("Marks-Only").unwrap(), Mode::MarksOnly);
        assert_eq!(Unmapped::parse("remove").unwrap(), Unmapped::Remove);
    }

    #[test]
    fn unknown_mode_is_an_error() {
        let err = Mode::parse("fold").unwrap_err();
        assert!(err.contains("unknown mode 'fold'"), "{err}");
        assert!(err.contains("marks-only"), "{err}");
    }

    #[test]
    fn unknown_policy_is_an_error() {
        let err = Unmapped::parse("delete").unwrap_err();
        assert!(err.contains("unknown unmapped policy 'delete'"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected_with_the_limit() {
        let text = "é".repeat(MAX_INPUT_CHARS + 1);
        let err = strip(&text, &Options::default()).unwrap_err();
        assert!(err.contains("200001 characters"), "{err}");
        assert!(err.contains("the limit is 200000"), "{err}");
    }

    #[test]
    fn exact_cap_is_accepted() {
        let text = "é".repeat(MAX_INPUT_CHARS);
        let r = strip(&text, &Options::default()).unwrap();
        assert_eq!(r.output_chars, MAX_INPUT_CHARS);
    }

    #[test]
    fn non_ascii_replacement_is_rejected() {
        let opts = Options {
            unmapped: Unmapped::Replace,
            replacement: "•".into(),
            ..Options::default()
        };
        let err = strip("𝕏", &opts).unwrap_err();
        assert!(err.contains("must be plain ASCII"), "{err}");
    }

    #[test]
    fn overlong_replacement_is_rejected() {
        let opts = Options {
            unmapped: Unmapped::Replace,
            replacement: "-".repeat(9),
            ..Options::default()
        };
        let err = strip("𝕏", &opts).unwrap_err();
        assert!(err.contains("the limit is 8"), "{err}");
    }
}
