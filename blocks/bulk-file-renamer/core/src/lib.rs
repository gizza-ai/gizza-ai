//! bulk-file-renamer core — pure batch-rename engine, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Takes a newline-separated list of filenames and one rename rule
//! (`find_replace`, `regex`, `sequential`, or `case`), plus optional prefix /
//! suffix / extension handling, and produces a deterministic old→new mapping.
//! It never touches the filesystem — it only computes new names, so it is safe
//! and reproducible for a client-side preview. (Re-zipping the actual bytes of
//! an uploaded archive is out of scope for this pure text engine.)

use regex::Regex;

/// A single rename entry: the original name and its computed replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rename {
    pub old: String,
    pub new: String,
}

/// The full result of a rename pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameResult {
    pub renames: Vec<Rename>,
    /// Number of NEW names that are duplicated across the mapping (a collision
    /// means two different originals would land on the same target name).
    pub collisions: usize,
}

/// Rename modes. `case_type` only matters for `Case`; `find`/`replace` for
/// `FindReplace`/`Regex`; the numbering params for `Sequential`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    FindReplace,
    Regex,
    Sequential,
    Case,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "find_replace" | "find-replace" | "" => Ok(Mode::FindReplace),
            "regex" => Ok(Mode::Regex),
            "sequential" | "number" | "numbering" => Ok(Mode::Sequential),
            "case" => Ok(Mode::Case),
            other => Err(format!(
                "unknown mode '{other}' (expected find_replace, regex, sequential, or case)"
            )),
        }
    }
}

/// Every option the engine understands. Build with sensible defaults and
/// override what the caller supplies.
#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub find: String,
    pub replace: String,
    pub case_type: String,
    /// Sequential pattern with `{n}` (number), `{name}` (original stem) and
    /// `{ext}` (original extension, no dot) tokens.
    pub pattern: String,
    pub start: i64,
    pub padding: usize,
    pub prefix: String,
    pub suffix: String,
    /// Apply the transform to the stem only and keep the original extension.
    pub preserve_extension: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::FindReplace,
            find: String::new(),
            replace: String::new(),
            case_type: "lower".into(),
            pattern: "file-{n}".into(),
            start: 1,
            padding: 1,
            prefix: String::new(),
            suffix: String::new(),
            preserve_extension: true,
        }
    }
}

/// Split a filename into (stem, extension-with-dot). A leading dot (dotfile) is
/// NOT treated as an extension; `archive.tar.gz` splits at the last dot.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// Split a string into word tokens, breaking on non-alphanumerics and on
/// lower/digit→upper camelCase boundaries.
fn tokenize(s: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            if let Some(last) = cur.chars().last() {
                if (last.is_lowercase() || last.is_ascii_digit()) && c.is_uppercase() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            cur.push(c);
        } else if !cur.is_empty() {
            words.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn capitalize(word: &str) -> String {
    let mut cs = word.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Title-case: lowercase everything, then uppercase the first letter of each
/// alphanumeric run. Separators (spaces, `_`, `-`, `.`) are preserved.
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_boundary = true;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if at_boundary {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_boundary = false;
        } else {
            out.push(c);
            at_boundary = true;
        }
    }
    out
}

fn apply_case(s: &str, case_type: &str) -> Result<String, String> {
    let out = match case_type.trim().to_ascii_lowercase().as_str() {
        "lower" | "" => s.to_lowercase(),
        "upper" => s.to_uppercase(),
        "title" => title_case(s),
        "snake" => tokenize(s)
            .iter()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join("_"),
        "kebab" => tokenize(s)
            .iter()
            .map(|w| w.to_lowercase())
            .collect::<Vec<_>>()
            .join("-"),
        "camel" => {
            let words = tokenize(s);
            words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    if i == 0 {
                        w.to_lowercase()
                    } else {
                        capitalize(w)
                    }
                })
                .collect::<String>()
        }
        "pascal" => tokenize(s).iter().map(|w| capitalize(w)).collect::<String>(),
        other => {
            return Err(format!(
                "unknown case '{other}' (expected lower, upper, title, snake, kebab, camel, or pascal)"
            ))
        }
    };
    Ok(out)
}

/// Compute the rename mapping for a newline-separated list of filenames. Blank
/// lines are ignored; surrounding whitespace on each name is trimmed.
pub fn rename(filenames: &str, opts: &Options) -> Result<RenameResult, String> {
    let names: Vec<&str> = filenames
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if names.is_empty() {
        return Err("no filenames provided (enter one filename per line)".into());
    }
    if opts.padding > 20 {
        return Err("padding must be 20 or fewer digits".into());
    }

    // Pre-compile the regex once for the whole batch.
    let re = if opts.mode == Mode::Regex {
        if opts.find.is_empty() {
            return Err("regex mode needs a `find` pattern".into());
        }
        Some(Regex::new(&opts.find).map_err(|e| format!("invalid regular expression: {e}"))?)
    } else {
        None
    };

    let mut renames = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let (stem, ext_dot) = split_ext(name);
        // ext without the leading dot, for the {ext} token.
        let ext_bare = ext_dot.strip_prefix('.').unwrap_or(ext_dot);

        let new = match opts.mode {
            Mode::Sequential => {
                let num = opts.start + i as i64;
                let number = format!("{:0>width$}", num, width = opts.padding.max(1));
                let expanded = opts
                    .pattern
                    .replace("{n}", &number)
                    .replace("{name}", stem)
                    .replace("{ext}", ext_bare);
                let mut base = format!("{}{}{}", opts.prefix, expanded, opts.suffix);
                // Auto-append the original extension when the pattern didn't
                // place one itself and preservation is on.
                if opts.preserve_extension && !opts.pattern.contains("{ext}") {
                    base.push_str(ext_dot);
                }
                base
            }
            _ => {
                // The transform target is the stem (extension preserved) or the
                // whole name (extension folded in).
                let (base, tail) = if opts.preserve_extension {
                    (stem, ext_dot)
                } else {
                    (*name, "")
                };
                let transformed = match opts.mode {
                    Mode::FindReplace => {
                        if opts.find.is_empty() {
                            base.to_string()
                        } else {
                            base.replace(&opts.find, &opts.replace)
                        }
                    }
                    Mode::Regex => re
                        .as_ref()
                        .unwrap()
                        .replace_all(base, opts.replace.as_str())
                        .into_owned(),
                    Mode::Case => apply_case(base, &opts.case_type)?,
                    Mode::Sequential => unreachable!(),
                };
                format!("{}{}{}{}", opts.prefix, transformed, opts.suffix, tail)
            }
        };

        renames.push(Rename {
            old: name.to_string(),
            new,
        });
    }

    // Count collisions: new names that appear more than once.
    let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for r in &renames {
        *seen.entry(r.new.as_str()).or_insert(0) += 1;
    }
    let collisions = renames.iter().filter(|r| seen[r.new.as_str()] > 1).count();

    Ok(RenameResult { renames, collisions })
}

/// Render a [`RenameResult`] as human-readable `old -> new` lines, with a
/// trailing warning line when any target name collides.
pub fn render(result: &RenameResult) -> String {
    let mut out = result
        .renames
        .iter()
        .map(|r| format!("{} -> {}", r.old, r.new))
        .collect::<Vec<_>>()
        .join("\n");
    if result.collisions > 0 {
        out.push_str(&format!(
            "\n\n⚠ {} name collision(s): different files map to the same new name.",
            result.collisions
        ));
    }
    out
}

/// Convenience: run a rename pass and return the rendered mapping text.
pub fn rename_text(filenames: &str, opts: &Options) -> Result<String, String> {
    Ok(render(&rename(filenames, opts)?))
}

/// Build [`Options`] from stringly-typed params (as the CLI/chat/web surfaces
/// supply them) and return the rendered mapping. `mode` and `case_type` are
/// validated; the numeric params fall back to their defaults when out of range.
#[allow(clippy::too_many_arguments)]
pub fn run_named(
    filenames: &str,
    mode: &str,
    find: &str,
    replace: &str,
    case_type: &str,
    pattern: &str,
    start: i64,
    padding: i64,
    prefix: &str,
    suffix: &str,
    preserve_extension: bool,
) -> Result<String, String> {
    let defaults = Options::default();
    let opts = Options {
        mode: Mode::parse(mode)?,
        find: find.to_string(),
        replace: replace.to_string(),
        case_type: if case_type.trim().is_empty() {
            defaults.case_type
        } else {
            case_type.to_string()
        },
        pattern: if pattern.trim().is_empty() {
            defaults.pattern
        } else {
            pattern.to_string()
        },
        start,
        padding: if padding >= 1 { padding as usize } else { 1 },
        prefix: prefix.to_string(),
        suffix: suffix.to_string(),
        preserve_extension,
    };
    rename_text(filenames, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(mode: Mode) -> Options {
        Options {
            mode,
            ..Default::default()
        }
    }

    #[test]
    fn find_replace_stem_only_preserves_ext() {
        let mut o = opts(Mode::FindReplace);
        o.find = "IMG".into();
        o.replace = "photo".into();
        let r = rename("IMG_001.JPG\nIMG_002.JPG", &o).unwrap();
        assert_eq!(r.renames[0].new, "photo_001.JPG");
        assert_eq!(r.renames[1].new, "photo_002.JPG");
        assert_eq!(r.collisions, 0);
    }

    #[test]
    fn find_replace_whole_name_when_not_preserving() {
        let mut o = opts(Mode::FindReplace);
        o.find = "JPG".into();
        o.replace = "jpg".into();
        o.preserve_extension = false;
        let r = rename("a.JPG", &o).unwrap();
        assert_eq!(r.renames[0].new, "a.jpg");
    }

    #[test]
    fn regex_capture_groups() {
        let mut o = opts(Mode::Regex);
        o.find = r"(\d{4})-(\d{2})-(\d{2})".into();
        o.replace = "${3}_${2}_${1}".into();
        let r = rename("2026-07-17-report.pdf", &o).unwrap();
        assert_eq!(r.renames[0].new, "17_07_2026-report.pdf");
    }

    #[test]
    fn sequential_pads_and_keeps_ext() {
        let mut o = opts(Mode::Sequential);
        o.pattern = "vacation-{n}".into();
        o.start = 1;
        o.padding = 3;
        let r = rename("a.png\nb.png\nc.png", &o).unwrap();
        assert_eq!(r.renames[0].new, "vacation-001.png");
        assert_eq!(r.renames[2].new, "vacation-003.png");
    }

    #[test]
    fn sequential_ext_token_places_extension() {
        let mut o = opts(Mode::Sequential);
        o.pattern = "{name}-{n}.{ext}".into();
        o.start = 5;
        let r = rename("song.mp3", &o).unwrap();
        assert_eq!(r.renames[0].new, "song-5.mp3");
    }

    #[test]
    fn case_modes() {
        let mut o = opts(Mode::Case);
        o.case_type = "snake".into();
        let r = rename("My Report File.txt", &o).unwrap();
        assert_eq!(r.renames[0].new, "my_report_file.txt");

        o.case_type = "kebab".into();
        assert_eq!(
            rename("My Report.txt", &o).unwrap().renames[0].new,
            "my-report.txt"
        );

        o.case_type = "upper".into();
        assert_eq!(rename("hello.txt", &o).unwrap().renames[0].new, "HELLO.txt");

        o.case_type = "pascal".into();
        assert_eq!(rename("my file.txt", &o).unwrap().renames[0].new, "MyFile.txt");
    }

    #[test]
    fn prefix_suffix_apply() {
        let mut o = opts(Mode::FindReplace);
        o.prefix = "2026_".into();
        o.suffix = "_final".into();
        let r = rename("draft.docx", &o).unwrap();
        assert_eq!(r.renames[0].new, "2026_draft_final.docx");
    }

    #[test]
    fn dotfiles_have_no_extension() {
        let mut o = opts(Mode::Case);
        o.case_type = "upper".into();
        let r = rename(".gitignore", &o).unwrap();
        assert_eq!(r.renames[0].new, ".GITIGNORE");
    }

    #[test]
    fn collision_detection() {
        let mut o = opts(Mode::Sequential);
        o.pattern = "same".into();
        o.preserve_extension = false;
        let r = rename("a.txt\nb.txt", &o).unwrap();
        assert_eq!(r.collisions, 2);
        assert!(render(&r).contains("collision"));
    }

    #[test]
    fn render_format() {
        let mut o = opts(Mode::FindReplace);
        o.find = "a".into();
        o.replace = "b".into();
        let text = rename_text("a1.txt\na2.txt", &o).unwrap();
        assert_eq!(text, "a1.txt -> b1.txt\na2.txt -> b2.txt");
    }

    #[test]
    fn errors() {
        assert!(rename("", &opts(Mode::FindReplace)).is_err()); // no names
        assert!(rename("  \n \n", &opts(Mode::FindReplace)).is_err()); // blank only
        let mut o = opts(Mode::Regex);
        o.find = "(".into();
        assert!(rename("a.txt", &o).is_err()); // bad regex
        let mut o = opts(Mode::Regex);
        o.find = "".into();
        assert!(rename("a.txt", &o).is_err()); // empty regex find
        let mut o = opts(Mode::Case);
        o.case_type = "bogus".into();
        assert!(rename("a.txt", &o).is_err()); // bad case
    }

    #[test]
    fn mode_parse() {
        assert_eq!(Mode::parse("find_replace").unwrap(), Mode::FindReplace);
        assert_eq!(Mode::parse("regex").unwrap(), Mode::Regex);
        assert_eq!(Mode::parse("sequential").unwrap(), Mode::Sequential);
        assert_eq!(Mode::parse("case").unwrap(), Mode::Case);
        assert!(Mode::parse("nope").is_err());
    }
}
