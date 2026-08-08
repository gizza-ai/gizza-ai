//! mv-script-generator core — pure compute, shared by the chat skill block and the web page.
//!
//! Turns a before→after filename mapping into a reviewable rename script (bash
//! `mv` or PowerShell `Rename-Item`/`Move-Item`) plus the matching undo script.
//! Nothing is ever moved here: the output is text.
//!
//! The interesting parts are the safety passes:
//! - every path is quoted with dialect-correct escaping and passed after `--` /
//!   via `-LiteralPath`, so spaces, quotes, `$` and leading dashes are inert;
//! - duplicate sources and duplicate destinations are hard errors (they would
//!   silently destroy a file);
//! - chained renames (`a→b`, `b→c`) are topologically ordered so the script can
//!   never clobber a file it still has to move;
//! - true cycles (`a→b`, `b→a`) are staged through a temp name and completed at
//!   the end — and the undo script unwinds through the same temp name.

use std::collections::{HashMap, HashSet};

/// Maximum number of rename pairs accepted in one run.
pub const MAX_PAIRS: usize = 1000;

/// Header words that make a first row a header rather than a rename pair.
const HEADER_WORDS: [&str; 16] = [
    "old",
    "new",
    "from",
    "to",
    "source",
    "destination",
    "dest",
    "target",
    "old_name",
    "new_name",
    "oldname",
    "newname",
    "original",
    "renamed",
    "current",
    "before",
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Delim {
    Arrow,
    Tab,
    Pipe,
    Csv,
}

impl Delim {
    fn split(self, line: &str) -> Option<(String, String)> {
        match self {
            Delim::Arrow => line
                .split_once("->")
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string())),
            Delim::Tab => line
                .split_once('\t')
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string())),
            Delim::Pipe => line
                .split_once('|')
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string())),
            Delim::Csv => {
                let fields = split_csv(line)?;
                if fields.len() != 2 {
                    return None;
                }
                let mut it = fields.into_iter();
                Some((it.next().unwrap(), it.next().unwrap()))
            }
        }
    }
}

/// Split one CSV line into fields, honouring `"quoted,fields"` and `""` escapes.
/// Unquoted fields are trimmed; quoted fields keep their inner whitespace.
/// Returns `None` for an unterminated quote.
fn split_csv(line: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut was_quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            in_quotes = true;
            was_quoted = true;
            cur.clear();
        } else if c == ',' {
            out.push(if was_quoted {
                std::mem::take(&mut cur)
            } else {
                cur.trim().to_string()
            });
            cur.clear();
            was_quoted = false;
        } else {
            cur.push(c);
        }
    }
    if in_quotes {
        return None;
    }
    out.push(if was_quoted { cur } else { cur.trim().to_string() });
    Some(out)
}

/// A content line of the mapping: the raw text plus its 1-based line number.
struct RawLine {
    number: usize,
    text: String,
}

fn content_lines(mapping: &str) -> Vec<RawLine> {
    mapping
        .lines()
        .enumerate()
        .filter_map(|(i, raw)| {
            let text = raw.trim_end_matches('\r').trim().to_string();
            if text.is_empty() || text.starts_with('#') {
                None
            } else {
                Some(RawLine {
                    number: i + 1,
                    text,
                })
            }
        })
        .collect()
}

fn parse_delim(format: &str) -> Result<Option<Delim>, String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(None),
        "csv" => Ok(Some(Delim::Csv)),
        "tsv" | "tab" => Ok(Some(Delim::Tab)),
        "arrow" => Ok(Some(Delim::Arrow)),
        "pipe" => Ok(Some(Delim::Pipe)),
        other => Err(format!(
            "unknown format '{other}' — use auto, csv, tsv, arrow, or pipe"
        )),
    }
}

fn detect_delim(lines: &[RawLine]) -> Result<Delim, String> {
    for cand in [Delim::Arrow, Delim::Tab, Delim::Pipe, Delim::Csv] {
        let ok = lines.iter().all(|l| {
            cand.split(&l.text)
                .is_some_and(|(a, b)| !a.is_empty() && !b.is_empty())
        });
        if ok {
            return Ok(cand);
        }
    }
    Err(format!(
        "could not detect the mapping delimiter on line {} — write each pair as 'old,new', \
         'old<TAB>new', 'old | new', or 'old -> new', or set format explicitly",
        lines[0].number
    ))
}

/// One before→after pair.
struct Pair {
    src: String,
    dst: String,
}

/// A single move the emitted script performs, in safe execution order.
struct Move {
    src: String,
    dst: String,
}

/// Counts surfaced in the script header comment.
#[derive(Default)]
struct Stats {
    skipped_unchanged: usize,
    reordered: bool,
    temps: usize,
}

fn parse_pairs(mapping: &str, format: &str) -> Result<(Vec<Pair>, Stats), String> {
    let lines = content_lines(mapping);
    if lines.is_empty() {
        return Err(
            "mapping is empty — provide one 'old,new' pair per line (blank lines and lines \
             starting with # are ignored)"
                .into(),
        );
    }
    let delim = match parse_delim(format)? {
        Some(d) => d,
        None => detect_delim(&lines)?,
    };

    let mut pairs: Vec<Pair> = Vec::new();
    let mut stats = Stats::default();
    let mut header_skipped = false;
    let mut by_src: HashMap<String, usize> = HashMap::new();
    let mut by_dst: HashMap<String, usize> = HashMap::new();

    for (idx, line) in lines.iter().enumerate() {
        let (src, dst) = delim.split(&line.text).ok_or_else(|| {
            format!(
                "line {}: could not split '{}' into an old and a new name",
                line.number, line.text
            )
        })?;
        // A recognised header row on the very first line is not a rename.
        if idx == 0
            && HEADER_WORDS.contains(&src.to_ascii_lowercase().as_str())
            && HEADER_WORDS.contains(&dst.to_ascii_lowercase().as_str())
        {
            header_skipped = true;
            continue;
        }
        if src.is_empty() || dst.is_empty() {
            return Err(format!(
                "line {}: both the old and the new name are required",
                line.number
            ));
        }
        for (label, value) in [("old", &src), ("new", &dst)] {
            if value.chars().any(char::is_control) {
                return Err(format!(
                    "line {}: the {label} name contains a control character, which cannot be \
                     written safely into a script",
                    line.number
                ));
            }
        }
        if src == dst {
            stats.skipped_unchanged += 1;
            continue;
        }
        if let Some(prev) = by_src.get(&src) {
            return Err(format!(
                "line {}: '{}' is renamed twice (also on line {})",
                line.number, src, prev
            ));
        }
        if let Some(prev) = by_dst.get(&dst) {
            return Err(format!(
                "line {}: two files would both become '{}' (also on line {}) — one would be \
                 destroyed",
                line.number, dst, prev
            ));
        }
        by_src.insert(src.clone(), line.number);
        by_dst.insert(dst.clone(), line.number);
        pairs.push(Pair { src, dst });
        if pairs.len() > MAX_PAIRS {
            return Err(format!(
                "too many rename pairs on line {} — the maximum is {MAX_PAIRS}",
                line.number
            ));
        }
    }

    if pairs.is_empty() {
        if stats.skipped_unchanged > 0 {
            return Err(format!(
                "nothing to rename — {} pair(s) map a name to itself and no other pair was found",
                stats.skipped_unchanged
            ));
        }
        if header_skipped {
            return Err(
                "nothing to rename — the mapping holds only a header row; add one 'old,new' pair \
                 per line below it"
                    .into(),
            );
        }
        return Err("nothing to rename — no old,new pair was found in the mapping".into());
    }
    Ok((pairs, stats))
}

/// Order the pairs so no move ever clobbers a file a later move still needs,
/// staging cycles through `.mvtmpN` temp names.
fn sequence(pairs: Vec<Pair>, stats: &mut Stats) -> Vec<Move> {
    let mut live_srcs: HashSet<String> = pairs.iter().map(|p| p.src.clone()).collect();
    let mut pending: Vec<Option<Pair>> = pairs.into_iter().map(Some).collect();
    let mut ops: Vec<Move> = Vec::new();
    let mut deferred: Vec<Move> = Vec::new();
    let mut emitted: Vec<usize> = Vec::new();

    loop {
        let mut progress = false;
        for i in 0..pending.len() {
            let ready = match &pending[i] {
                Some(p) => !live_srcs.contains(&p.dst),
                None => false,
            };
            if ready {
                let p = pending[i].take().unwrap();
                live_srcs.remove(&p.src);
                ops.push(Move {
                    src: p.src,
                    dst: p.dst,
                });
                emitted.push(i);
                progress = true;
            }
        }
        if progress {
            continue;
        }
        // Everything left is part of a cycle: stage one member through a temp
        // name so the rest of its cycle unblocks.
        match pending.iter().position(Option::is_some) {
            None => break,
            Some(i) => {
                let p = pending[i].take().unwrap();
                stats.temps += 1;
                let tmp = format!("{}.mvtmp{}", p.src, stats.temps);
                live_srcs.remove(&p.src);
                ops.push(Move {
                    src: p.src,
                    dst: tmp.clone(),
                });
                emitted.push(i);
                deferred.push(Move { src: tmp, dst: p.dst });
            }
        }
    }

    stats.reordered = emitted.windows(2).any(|w| w[0] > w[1]);
    ops.extend(deferred);
    ops
}

/// Quote a path for POSIX shells: wrap in single quotes, ending and re-opening
/// the quoting around each embedded single quote.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Quote a path for PowerShell: single-quoted literal, `'` doubled.
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

struct Opts<'a> {
    base_dir: &'a str,
    dry_run: bool,
    overwrite: bool,
    mkdir_parents: bool,
    comments: bool,
}

fn header(kind: &str, count: usize, stats: &Stats, o: &Opts, undo: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# {kind} script generated by mv-script-generator — {count} rename{}.\n",
        if count == 1 { "" } else { "s" }
    ));
    let mut notes: Vec<String> = Vec::new();
    if stats.skipped_unchanged > 0 && !undo {
        notes.push(format!(
            "{} pair(s) skipped because the name is unchanged",
            stats.skipped_unchanged
        ));
    }
    if stats.reordered {
        notes.push("moves reordered so none clobbers a file a later move needs".into());
    }
    if stats.temps > 0 {
        notes.push(format!(
            "{} temp name(s) used to break a rename cycle",
            stats.temps
        ));
    }
    if o.overwrite {
        notes.push("overwrite enabled: an existing destination is replaced".into());
    }
    if o.dry_run {
        notes.push("dry run: each move is reported, not performed".into());
    }
    if !notes.is_empty() {
        out.push_str(&format!("# Notes: {}.\n", notes.join("; ")));
    }
    out.push_str("# Review this script before running it. Nothing moves until you run it.\n");
    out
}

fn bash_script(kind: &str, ops: &[Move], stats: &Stats, o: &Opts, undo: bool) -> String {
    let mut s = String::from("#!/usr/bin/env bash\n");
    if o.comments {
        s.push_str(&header(kind, ops.len(), stats, o, undo));
    }
    s.push_str("set -euo pipefail\n\n");

    if !o.base_dir.is_empty() {
        s.push_str(&format!("cd -- {}\n\n", sh_quote(o.base_dir)));
    }

    s.push_str("mv_safe() {\n  local src=$1 dst=$2\n");
    if o.dry_run {
        s.push_str("  printf 'would move: %s -> %s\\n' \"$src\" \"$dst\"\n");
    } else {
        s.push_str(
            "  if [ ! -e \"$src\" ] && [ ! -L \"$src\" ]; then\n\
             \x20   printf 'mv-script: missing source: %s\\n' \"$src\" >&2\n\
             \x20   exit 1\n\
             \x20 fi\n",
        );
        if !o.overwrite {
            s.push_str(
                "  if [ -e \"$dst\" ] || [ -L \"$dst\" ]; then\n\
                 \x20   printf 'mv-script: destination exists: %s\\n' \"$dst\" >&2\n\
                 \x20   exit 1\n\
                 \x20 fi\n",
            );
        }
        if o.mkdir_parents {
            s.push_str("  mkdir -p -- \"$(dirname -- \"$dst\")\"\n");
        }
        s.push_str(if o.overwrite {
            "  mv -f -- \"$src\" \"$dst\"\n"
        } else {
            "  mv -- \"$src\" \"$dst\"\n"
        });
    }
    s.push_str("}\n\n");

    for m in ops {
        s.push_str(&format!(
            "mv_safe {} {}\n",
            sh_quote(&m.src),
            sh_quote(&m.dst)
        ));
    }
    s
}

fn ps_script(kind: &str, ops: &[Move], stats: &Stats, o: &Opts, undo: bool) -> String {
    let mut s = String::from("#Requires -Version 5.1\n");
    if o.comments {
        s.push_str(&header(kind, ops.len(), stats, o, undo));
    }
    s.push_str("$ErrorActionPreference = 'Stop'\n\n");

    if !o.base_dir.is_empty() {
        s.push_str(&format!(
            "Set-Location -LiteralPath {}\n\n",
            ps_quote(o.base_dir)
        ));
    }

    s.push_str(
        "function Move-Safe {\n\
         \x20   param([string]$Source, [string]$Destination)\n",
    );
    let force = if o.overwrite { " -Force" } else { "" };
    if o.dry_run {
        s.push_str(&format!(
            "    if ((Split-Path -Parent -Path $Source) -eq (Split-Path -Parent -Path $Destination)) {{\n\
             \x20       Rename-Item -LiteralPath $Source -NewName (Split-Path -Leaf -Path $Destination){force} -WhatIf\n\
             \x20   }} else {{\n\
             \x20       Move-Item -LiteralPath $Source -Destination $Destination{force} -WhatIf\n\
             \x20   }}\n"
        ));
    } else {
        s.push_str(
            "    if (-not (Test-Path -LiteralPath $Source)) {\n\
             \x20       throw \"mv-script: missing source: $Source\"\n\
             \x20   }\n",
        );
        if !o.overwrite {
            s.push_str(
                "    if (Test-Path -LiteralPath $Destination) {\n\
                 \x20       throw \"mv-script: destination exists: $Destination\"\n\
                 \x20   }\n",
            );
        }
        s.push_str("    $parent = Split-Path -Parent -Path $Destination\n");
        if o.mkdir_parents {
            s.push_str(
                "    if ($parent -and -not (Test-Path -LiteralPath $parent)) {\n\
                 \x20       New-Item -ItemType Directory -Path $parent -Force | Out-Null\n\
                 \x20   }\n",
            );
        }
        s.push_str(&format!(
            "    if ((Split-Path -Parent -Path $Source) -eq $parent) {{\n\
             \x20       Rename-Item -LiteralPath $Source -NewName (Split-Path -Leaf -Path $Destination){force}\n\
             \x20   }} else {{\n\
             \x20       Move-Item -LiteralPath $Source -Destination $Destination{force}\n\
             \x20   }}\n"
        ));
    }
    s.push_str("}\n\n");

    for m in ops {
        s.push_str(&format!(
            "Move-Safe {} {}\n",
            ps_quote(&m.src),
            ps_quote(&m.dst)
        ));
    }
    s
}

/// Build the rename script (and, when `undo_script` is set, the undo script)
/// for a before→after filename mapping.
///
/// `format`: `auto` | `csv` | `tsv` | `arrow` | `pipe`.
/// `shell`:  `bash` | `powershell`.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    mapping: &str,
    format: &str,
    shell: &str,
    base_dir: &str,
    dry_run: bool,
    overwrite: bool,
    mkdir_parents: bool,
    undo_script: bool,
    comments: bool,
) -> Result<String, String> {
    let shell_kind = shell.trim().to_ascii_lowercase();
    let shell_kind = match shell_kind.as_str() {
        "" | "bash" | "sh" => "bash",
        "powershell" | "pwsh" | "ps1" => "powershell",
        other => {
            return Err(format!(
                "unknown shell '{other}' — use bash or powershell"
            ))
        }
    };

    let base_dir = base_dir.trim();
    if base_dir.chars().any(char::is_control) {
        return Err("base_dir contains a control character".into());
    }

    let (pairs, mut stats) = parse_pairs(mapping, format)?;
    let ops = sequence(pairs, &mut stats);

    let undo_ops: Vec<Move> = ops
        .iter()
        .rev()
        .map(|m| Move {
            src: m.dst.clone(),
            dst: m.src.clone(),
        })
        .collect();

    let opts = Opts {
        base_dir,
        dry_run,
        overwrite,
        mkdir_parents,
        comments,
    };
    // Undo must never be blocked by the forward run's overwrite choice being
    // off, but it must also not silently clobber: it keeps the same guards.
    let (ext, main, undo) = if shell_kind == "bash" {
        (
            "sh",
            bash_script("Rename", &ops, &stats, &opts, false),
            bash_script("Undo", &undo_ops, &stats, &opts, true),
        )
    } else {
        (
            "ps1",
            ps_script("Rename", &ops, &stats, &opts, false),
            ps_script("Undo", &undo_ops, &stats, &opts, true),
        )
    };

    if !undo_script {
        return Ok(main);
    }
    Ok(format!(
        "# ===== rename.{ext} =====\n{main}\n# ===== undo-rename.{ext} =====\n{undo}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(mapping: &str) -> String {
        generate(mapping, "auto", "bash", "", false, false, true, false, true).unwrap()
    }

    #[test]
    fn happy_path_csv_bash() {
        let out = gen("IMG_001.JPG,photo-001.jpg\nIMG_002.JPG,photo-002.jpg");
        assert!(out.starts_with("#!/usr/bin/env bash\n"));
        assert!(out.contains("# Rename script generated by mv-script-generator — 2 renames."));
        assert!(out.contains("set -euo pipefail"));
        assert!(out.contains("mv_safe 'IMG_001.JPG' 'photo-001.jpg'"));
        assert!(out.contains("mv_safe 'IMG_002.JPG' 'photo-002.jpg'"));
        assert!(out.contains("mv -- \"$src\" \"$dst\""));
    }

    #[test]
    fn empty_mapping_is_an_error() {
        let err = generate("   \n# just a comment", "auto", "bash", "", false, false, true, true, true)
            .unwrap_err();
        assert!(err.contains("mapping is empty"), "{err}");
    }

    #[test]
    fn unsplittable_line_is_an_error() {
        let err =
            generate("just-one-name.txt", "auto", "bash", "", false, false, true, true, true)
                .unwrap_err();
        assert!(err.contains("could not detect the mapping delimiter"), "{err}");
    }

    #[test]
    fn duplicate_destination_is_an_error() {
        let err = generate("a.txt,c.txt\nb.txt,c.txt", "csv", "bash", "", false, false, true, true, true)
            .unwrap_err();
        assert!(err.contains("would both become 'c.txt'"), "{err}");
        assert!(err.contains("line 1"), "{err}");
    }

    #[test]
    fn duplicate_source_is_an_error() {
        let err = generate("a.txt,b.txt\na.txt,c.txt", "csv", "bash", "", false, false, true, true, true)
            .unwrap_err();
        assert!(err.contains("renamed twice"), "{err}");
    }

    #[test]
    fn unchanged_pairs_are_skipped_and_reported() {
        let out = gen("a.txt,a.txt\nb.txt,c.txt");
        assert!(out.contains("— 1 rename."), "{out}");
        assert!(out.contains("1 pair(s) skipped because the name is unchanged"), "{out}");
        assert!(!out.contains("mv_safe 'a.txt' 'a.txt'"));
    }

    #[test]
    fn all_unchanged_is_an_error() {
        let err = generate("a.txt,a.txt", "csv", "bash", "", false, false, true, true, true)
            .unwrap_err();
        assert!(err.contains("nothing to rename"), "{err}");
    }

    #[test]
    fn chained_renames_are_reordered() {
        // a→b must run AFTER b→c, or b is destroyed.
        let out = gen("a.txt,b.txt\nb.txt,c.txt");
        let first = out.find("mv_safe 'b.txt' 'c.txt'").unwrap();
        let second = out.find("mv_safe 'a.txt' 'b.txt'").unwrap();
        assert!(first < second, "{out}");
        assert!(out.contains("moves reordered"), "{out}");
    }

    #[test]
    fn swap_cycle_uses_a_temp_name() {
        let out = gen("a.txt,b.txt\nb.txt,a.txt");
        assert!(out.contains("mv_safe 'a.txt' 'a.txt.mvtmp1'"), "{out}");
        assert!(out.contains("mv_safe 'b.txt' 'a.txt'"), "{out}");
        assert!(out.contains("mv_safe 'a.txt.mvtmp1' 'b.txt'"), "{out}");
        assert!(out.contains("1 temp name(s) used to break a rename cycle"), "{out}");
    }

    #[test]
    fn undo_script_reverses_every_move_in_reverse_order() {
        let out = generate(
            "a.txt,b.txt\nb.txt,a.txt",
            "csv",
            "bash",
            "",
            false,
            false,
            true,
            true,
            true,
        )
        .unwrap();
        let (fwd, undo) = out.split_once("# ===== undo-rename.sh =====").unwrap();
        assert!(fwd.starts_with("# ===== rename.sh ====="));
        // Forward: a→tmp, b→a, tmp→b. Undo: b→tmp, a→b, tmp→a.
        let i1 = undo.find("mv_safe 'b.txt' 'a.txt.mvtmp1'").unwrap();
        let i2 = undo.find("mv_safe 'a.txt' 'b.txt'").unwrap();
        let i3 = undo.find("mv_safe 'a.txt.mvtmp1' 'a.txt'").unwrap();
        assert!(i1 < i2 && i2 < i3, "{undo}");
    }

    #[test]
    fn quoting_survives_spaces_quotes_and_dollars() {
        let out = gen("my file's $HOME.txt\tclean-name.txt");
        assert!(out.contains(r"mv_safe 'my file'\''s $HOME.txt' 'clean-name.txt'"), "{out}");
    }

    #[test]
    fn leading_dash_paths_are_passed_after_a_double_dash() {
        let out = gen("-weird.txt,ok.txt");
        assert!(out.contains("mv -- \"$src\" \"$dst\""), "{out}");
        assert!(out.contains("mv_safe '-weird.txt' 'ok.txt'"), "{out}");
    }

    #[test]
    fn arrow_tab_pipe_and_csv_all_auto_detect() {
        assert!(gen("a.txt -> b.txt").contains("mv_safe 'a.txt' 'b.txt'"));
        assert!(gen("a.txt\tb.txt").contains("mv_safe 'a.txt' 'b.txt'"));
        assert!(gen("a.txt | b.txt").contains("mv_safe 'a.txt' 'b.txt'"));
        assert!(gen("a.txt,b.txt").contains("mv_safe 'a.txt' 'b.txt'"));
    }

    #[test]
    fn csv_quoting_allows_commas_in_names() {
        let out = generate(
            "\"report, final.txt\",report-final.txt",
            "csv",
            "bash",
            "",
            false,
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert!(out.contains("mv_safe 'report, final.txt' 'report-final.txt'"), "{out}");
    }

    #[test]
    fn header_row_is_skipped() {
        let out = gen("old,new\na.txt,b.txt");
        assert!(out.contains("— 1 rename."), "{out}");
        assert!(!out.contains("mv_safe 'old' 'new'"), "{out}");
    }

    #[test]
    fn explicit_format_beats_detection() {
        // Contains an arrow, but csv was asked for explicitly.
        let out = generate("a->b.txt,c.txt", "csv", "bash", "", false, false, true, false, true)
            .unwrap();
        assert!(out.contains("mv_safe 'a->b.txt' 'c.txt'"), "{out}");
    }

    #[test]
    fn powershell_uses_rename_item_and_doubles_quotes() {
        let out = generate(
            "it's here.txt,there.txt",
            "csv",
            "powershell",
            "",
            false,
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert!(out.starts_with("#Requires -Version 5.1\n"));
        assert!(out.contains("$ErrorActionPreference = 'Stop'"));
        assert!(out.contains("Rename-Item -LiteralPath $Source -NewName"));
        assert!(out.contains("Move-Safe 'it''s here.txt' 'there.txt'"), "{out}");
    }

    #[test]
    fn dry_run_reports_instead_of_moving() {
        let bash = generate("a.txt,b.txt", "csv", "bash", "", true, false, true, false, true).unwrap();
        assert!(bash.contains("printf 'would move: %s -> %s\\n'"), "{bash}");
        assert!(!bash.contains("mv -- "), "{bash}");
        let ps =
            generate("a.txt,b.txt", "csv", "powershell", "", true, false, true, false, true).unwrap();
        assert!(ps.contains("-WhatIf"), "{ps}");
    }

    #[test]
    fn overwrite_drops_the_destination_guard() {
        let out = generate("a.txt,b.txt", "csv", "bash", "", false, true, true, false, true).unwrap();
        assert!(out.contains("mv -f -- "), "{out}");
        assert!(!out.contains("destination exists"), "{out}");
        let ps =
            generate("a.txt,b.txt", "csv", "powershell", "", false, true, true, false, true).unwrap();
        assert!(ps.contains("-Force"), "{ps}");
    }

    #[test]
    fn mkdir_parents_can_be_turned_off() {
        let on = generate("a.txt,sub/b.txt", "csv", "bash", "", false, false, true, false, true).unwrap();
        assert!(on.contains("mkdir -p -- "), "{on}");
        let off =
            generate("a.txt,sub/b.txt", "csv", "bash", "", false, false, false, false, true).unwrap();
        assert!(!off.contains("mkdir -p"), "{off}");
    }

    #[test]
    fn base_dir_is_quoted_and_emitted() {
        let out = generate(
            "a.txt,b.txt",
            "csv",
            "bash",
            "/tmp/my photos",
            false,
            false,
            true,
            false,
            true,
        )
        .unwrap();
        assert!(out.contains("cd -- '/tmp/my photos'"), "{out}");
    }

    #[test]
    fn comments_can_be_turned_off() {
        let out = generate("a.txt,b.txt", "csv", "bash", "", false, false, true, false, false).unwrap();
        assert!(!out.contains("# Rename script"), "{out}");
        assert!(out.contains("set -euo pipefail"), "{out}");
    }

    #[test]
    fn control_characters_are_rejected() {
        let err = generate("a\u{7}.txt,b.txt", "csv", "bash", "", false, false, true, true, true)
            .unwrap_err();
        assert!(err.contains("control character"), "{err}");
    }

    #[test]
    fn unknown_shell_and_format_are_rejected() {
        assert!(
            generate("a,b", "csv", "fish", "", false, false, true, true, true)
                .unwrap_err()
                .contains("unknown shell")
        );
        assert!(
            generate("a,b", "yaml", "bash", "", false, false, true, true, true)
                .unwrap_err()
                .contains("unknown format")
        );
    }

    #[test]
    fn pair_cap_is_enforced_at_the_boundary() {
        let ok: String = (0..MAX_PAIRS)
            .map(|i| format!("a{i}.txt,b{i}.txt\n"))
            .collect();
        assert!(generate(&ok, "csv", "bash", "", false, false, true, false, true).is_ok());
        let too_many: String = (0..=MAX_PAIRS)
            .map(|i| format!("a{i}.txt,b{i}.txt\n"))
            .collect();
        let err = generate(&too_many, "csv", "bash", "", false, false, true, false, true).unwrap_err();
        assert!(err.contains("too many rename pairs"), "{err}");
    }
}
