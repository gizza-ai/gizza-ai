//! phylip-to-fasta core — parse PHYLIP multiple-sequence-alignment text (both the
//! sequential and the interleaved layout, with strict 10-column or relaxed
//! whitespace-delimited taxon names) and emit standard FASTA.
//!
//! Pure compute; no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.
//!
//! A PHYLIP file opens with a count header — `<ntaxa> <nchar>` — followed by the
//! alignment itself:
//!
//! * **sequential** — each taxon's full sequence follows its name, optionally
//!   wrapped over several lines, before the next taxon starts;
//! * **interleaved** — the first `ntaxa` lines carry the names plus the first
//!   chunk of every sequence, and each later block appends the next chunk in the
//!   same taxon order.
//!
//! Both layouts and both name styles are auto-detected by parsing and checking
//! the result against the header's declared `nchar`.

/// Largest line-wrap width `wrap` may request. `wrap = 0` disables wrapping
/// (one sequence line per taxon). Bounds the descriptor so the LLM-/CLI-facing
/// schema can't drift from what `convert` enforces.
pub const MAX_WRAP: u32 = 1000;

/// Width of the taxon-name field in STRICT PHYLIP: the name is columns 1–10 and
/// the sequence data starts at column 11, with no separator required.
pub const STRICT_NAME_WIDTH: usize = 10;

/// How the alignment body is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Detect from the block structure, verified against the declared `nchar`.
    Auto,
    /// Each taxon's whole sequence appears before the next taxon's.
    Sequential,
    /// Sequences are split into blocks; every block holds one chunk per taxon.
    Interleaved,
}

/// How a taxon name is separated from its sequence data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStyle {
    /// Try relaxed, then strict; keep whichever matches the declared `nchar`.
    Auto,
    /// Strict PHYLIP: the name is exactly the first 10 columns.
    Strict,
    /// Relaxed PHYLIP (RAxML / PhyML style): the name is the first
    /// whitespace-delimited token, so it may be longer than 10 characters.
    Relaxed,
}

/// Options controlling the PHYLIP → FASTA conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Sequential vs interleaved body layout.
    pub layout: Layout,
    /// Strict 10-column vs relaxed whitespace-delimited taxon names.
    pub name_style: NameStyle,
    /// Wrap sequence lines at this many characters. `0` = one line per sequence.
    pub wrap: usize,
    /// Uppercase the sequence residues.
    pub uppercase: bool,
    /// Strip the alignment gap characters `-` and `.` (align → unaligned FASTA).
    pub remove_gaps: bool,
    /// Accept a file whose taxon count, sequence lengths or residue characters
    /// disagree with the header instead of reporting an error.
    pub tolerant: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: Layout::Auto,
            name_style: NameStyle::Auto,
            wrap: 60,
            uppercase: false,
            remove_gaps: false,
            tolerant: false,
        }
    }
}

/// One parsed taxon: its name and its (whitespace-stripped) sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub sequence: String,
}

/// Residue characters PHYLIP alignments are allowed to use: IUPAC letters, the
/// gap characters `-` and `.`, missing data `?`, a stop codon `*`, and the `~`
/// some writers use for a terminal gap. Digits are tolerated because a few
/// writers embed position counters.
fn is_residue(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '?' | '*' | '~')
}

/// Split the count header into `(ntaxa, nchar, interleaved_hint)`.
///
/// Some PHYLIP dialects append option letters after the two counts; a trailing
/// `I` is the interleaved flag, which we take as a detection hint.
fn parse_header(line: &str) -> Result<(usize, usize, Option<Layout>), String> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        return Err(format!(
            "not PHYLIP: the first line must be a count header '<taxa> <sites>' (e.g. '3 12'), found {line:?}"
        ));
    }
    let ntaxa: usize = tokens[0].parse().map_err(|_| {
        format!(
            "not PHYLIP: expected the taxon count as the first number of the header line, found {:?}",
            tokens[0]
        )
    })?;
    let nchar: usize = tokens[1].parse().map_err(|_| {
        format!(
            "not PHYLIP: expected the site (alignment length) count as the second number of the header line, found {:?}",
            tokens[1]
        )
    })?;
    if ntaxa == 0 {
        return Err("not PHYLIP: the header declares 0 taxa".to_string());
    }
    let hint = tokens.get(2).and_then(|t| match t.to_ascii_uppercase().as_str() {
        "I" => Some(Layout::Interleaved),
        "S" => Some(Layout::Sequential),
        _ => None,
    });
    Ok((ntaxa, nchar, hint))
}

/// Split a name line into `(name, sequence_chunk)` under the given name style.
fn split_name(line: &str, style: NameStyle) -> Result<(String, String), String> {
    match style {
        NameStyle::Strict => {
            let chars: Vec<char> = line.chars().collect();
            let cut = STRICT_NAME_WIDTH.min(chars.len());
            let name: String = chars[..cut].iter().collect::<String>().trim().to_string();
            let rest: String = chars[cut..].iter().filter(|c| !c.is_whitespace()).collect();
            Ok((name, rest))
        }
        NameStyle::Relaxed => {
            let trimmed = line.trim_start();
            let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            let name = trimmed[..end].to_string();
            let rest: String = trimmed[end..].chars().filter(|c| !c.is_whitespace()).collect();
            Ok((name, rest))
        }
        // `Auto` is resolved by the caller before a concrete parse runs.
        NameStyle::Auto => Err("internal: name style must be resolved before parsing".to_string()),
    }
}

/// Every non-blank line after the header, in order.
fn data_lines(lines: &[&str]) -> Vec<String> {
    lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect()
}

/// Parse the SEQUENTIAL layout: a name line starts a taxon, and following lines
/// extend it until `nchar` residues have accumulated.
fn parse_sequential(
    lines: &[&str],
    ntaxa: usize,
    nchar: usize,
    style: NameStyle,
) -> Result<Vec<Record>, String> {
    let data = data_lines(lines);
    let mut records: Vec<Record> = Vec::new();
    let mut idx = 0usize;
    while idx < data.len() && records.len() < ntaxa {
        let (name, first) = split_name(&data[idx], style)?;
        idx += 1;
        let mut sequence = first;
        // Keep pulling continuation lines until the declared width is reached.
        while sequence.chars().count() < nchar && idx < data.len() {
            sequence.extend(data[idx].chars().filter(|c| !c.is_whitespace()));
            idx += 1;
        }
        records.push(Record { name, sequence });
    }
    if idx < data.len() {
        return Err(format!(
            "malformed PHYLIP (sequential): {} unread line(s) after the {ntaxa} declared taxa — is the file interleaved?",
            data.len() - idx
        ));
    }
    Ok(records)
}

/// Parse the INTERLEAVED layout: the first `ntaxa` lines carry names plus the
/// first chunk; every later line appends to taxon `i % ntaxa`, in order.
fn parse_interleaved(
    lines: &[&str],
    ntaxa: usize,
    style: NameStyle,
) -> Result<Vec<Record>, String> {
    let data = data_lines(lines);
    if data.len() < ntaxa {
        return Err(format!(
            "malformed PHYLIP (interleaved): the header declares {ntaxa} taxa but only {} data line(s) follow",
            data.len()
        ));
    }
    let mut records: Vec<Record> = Vec::with_capacity(ntaxa);
    for line in &data[..ntaxa] {
        let (name, chunk) = split_name(line, style)?;
        records.push(Record { name, sequence: chunk });
    }
    let rest = &data[ntaxa..];
    if !rest.is_empty() && rest.len() % ntaxa != 0 {
        return Err(format!(
            "malformed PHYLIP (interleaved): {} continuation line(s) is not a multiple of the {ntaxa} declared taxa",
            rest.len()
        ));
    }
    for (i, line) in rest.iter().enumerate() {
        let taxon = i % ntaxa;
        // Some writers repeat the taxon name in every block — drop it when the
        // line starts with this taxon's own name.
        let name = &records[taxon].name;
        let trimmed = line.trim_start();
        let body = match trimmed.strip_prefix(name.as_str()) {
            Some(after) if !name.is_empty() && after.starts_with(char::is_whitespace) => after,
            _ => trimmed,
        };
        records[taxon]
            .sequence
            .extend(body.chars().filter(|c| !c.is_whitespace()));
    }
    Ok(records)
}

/// Check a parse against the header. Used both as the real validator and as the
/// scoring function that picks between the candidate layout/name-style parses.
fn validate(records: &[Record], ntaxa: usize, nchar: usize) -> Result<(), String> {
    if records.len() != ntaxa {
        return Err(format!(
            "malformed PHYLIP: the header declares {ntaxa} taxa but {} were parsed",
            records.len()
        ));
    }
    for (i, r) in records.iter().enumerate() {
        if r.name.is_empty() {
            return Err(format!(
                "malformed PHYLIP: taxon {} has an empty name — check the name style (strict names occupy columns 1-10)",
                i + 1
            ));
        }
        let len = r.sequence.chars().count();
        if len != nchar {
            return Err(format!(
                "malformed PHYLIP: taxon {} ({:?}) has {len} site(s) but the header declares {nchar}",
                i + 1,
                r.name
            ));
        }
        if let Some(bad) = r.sequence.chars().find(|c| !is_residue(*c)) {
            return Err(format!(
                "malformed PHYLIP: taxon {} ({:?}) contains the invalid residue character {bad:?} — expected letters, digits, or one of - . ? * ~",
                i + 1,
                r.name
            ));
        }
    }
    Ok(())
}

/// Decide which layout to try first from the block structure.
fn detect_layout(lines: &[&str], ntaxa: usize, hint: Option<Layout>) -> Layout {
    if let Some(h) = hint {
        return h;
    }
    let data = data_lines(lines);
    if data.len() == ntaxa {
        // One line per taxon reads identically either way; sequential is simpler.
        return Layout::Sequential;
    }
    // Blank-line-separated blocks whose first block is exactly one line per taxon
    // is the classic interleaved shape.
    let mut blocks: Vec<usize> = Vec::new();
    let mut current = 0usize;
    for line in lines {
        if line.trim().is_empty() {
            if current > 0 {
                blocks.push(current);
                current = 0;
            }
        } else {
            current += 1;
        }
    }
    if current > 0 {
        blocks.push(current);
    }
    if blocks.len() > 1 && blocks[0] == ntaxa {
        return Layout::Interleaved;
    }
    // No blank-line structure to go on: an exact multiple of ntaxa is more often
    // interleaved-without-separators than a wrapped sequential file, and the
    // caller re-checks the other layout against `nchar` anyway.
    if data.len() > ntaxa && data.len() % ntaxa == 0 {
        return Layout::Interleaved;
    }
    Layout::Sequential
}

/// Run one concrete (layout, name style) parse.
fn parse_one(
    lines: &[&str],
    ntaxa: usize,
    nchar: usize,
    layout: Layout,
    style: NameStyle,
) -> Result<Vec<Record>, String> {
    match layout {
        Layout::Interleaved => parse_interleaved(lines, ntaxa, style),
        _ => parse_sequential(lines, ntaxa, nchar, style),
    }
}

/// Parse PHYLIP text into records, resolving `Layout::Auto` / `NameStyle::Auto`
/// by trying every candidate combination and keeping the first that validates.
pub fn parse(input: &str, opts: &Options) -> Result<Vec<Record>, String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let all: Vec<&str> = normalized.split('\n').collect();
    let header_idx = match all.iter().position(|l| !l.trim().is_empty()) {
        Some(i) => i,
        None => return Ok(Vec::new()),
    };
    let (ntaxa, nchar, hint) = parse_header(all[header_idx])?;
    let body: Vec<&str> = all[header_idx + 1..].to_vec();

    // Candidate layouts, most likely first.
    let layouts: Vec<Layout> = match opts.layout {
        Layout::Auto => {
            let first = detect_layout(&body, ntaxa, hint);
            let other = if first == Layout::Interleaved {
                Layout::Sequential
            } else {
                Layout::Interleaved
            };
            vec![first, other]
        }
        l => vec![l],
    };
    // Candidate name styles, most likely first. Relaxed handles both the modern
    // long-name files and any strict file that separates name from data with
    // whitespace; strict is the fallback for fixed-column files.
    let styles: Vec<NameStyle> = match opts.name_style {
        NameStyle::Auto => vec![NameStyle::Relaxed, NameStyle::Strict],
        s => vec![s],
    };

    // The first combination that fully validates wins. Otherwise remember the
    // most-preferred parse that at least produced records (the tolerant-mode
    // fallback) and the first explanation of what went wrong (the error).
    let mut fallback: Option<Vec<Record>> = None;
    let mut first_err: Option<String> = None;
    for layout in &layouts {
        for style in &styles {
            match parse_one(&body, ntaxa, nchar, *layout, *style) {
                Ok(records) => match validate(&records, ntaxa, nchar) {
                    Ok(()) => return Ok(records),
                    Err(e) => {
                        if fallback.is_none() {
                            fallback = Some(records);
                        }
                        if first_err.is_none() {
                            first_err = Some(e);
                        }
                    }
                },
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }
    }

    if opts.tolerant {
        if let Some(records) = fallback {
            return Ok(records);
        }
    }
    Err(first_err
        .unwrap_or_else(|| "malformed PHYLIP: no taxa could be parsed from the file".to_string()))
}

/// Render records as FASTA text, applying the case / gap / wrap options.
fn render(records: &[Record], opts: &Options) -> String {
    let wrap = (opts.wrap as u32).min(MAX_WRAP) as usize;
    let mut out = String::new();
    for record in records {
        out.push('>');
        out.push_str(&record.name);
        out.push('\n');

        let mut seq: String = if opts.remove_gaps {
            record.sequence.chars().filter(|c| *c != '-' && *c != '.').collect()
        } else {
            record.sequence.clone()
        };
        if opts.uppercase {
            seq = seq.to_ascii_uppercase();
        }

        if wrap == 0 || seq.is_empty() {
            out.push_str(&seq);
            out.push('\n');
        } else {
            let chars: Vec<char> = seq.chars().collect();
            let mut start = 0usize;
            while start < chars.len() {
                let end = (start + wrap).min(chars.len());
                out.extend(chars[start..end].iter());
                out.push('\n');
                start = end;
            }
        }
    }
    out
}

/// Convert PHYLIP alignment text to FASTA text.
///
/// Returns `Err` when the count header is missing or unparseable, or (unless
/// `tolerant`) when no layout / name-style combination produces `ntaxa`
/// sequences of `nchar` valid residues. Empty input yields empty output.
pub fn convert(input: &str, opts: &Options) -> Result<String, String> {
    let records = parse(input, opts)?;
    Ok(render(&records, opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Classic interleaved PHYLIP: strict 10-column names, two blocks.
    const INTERLEAVED: &str = "\
3 12
Alpha     ACGT
Beta      ACGA
Gamma     TCGA

ACGTACGT
ACGTACGT
ACGTACGT
";

    /// Sequential PHYLIP with relaxed (long) names, one line per taxon.
    const SEQUENTIAL: &str = "\
2 8
Homo_sapiens ACGTACGT
Pan_troglodytes ACGTTCGT
";

    fn no_wrap() -> Options {
        Options { wrap: 0, ..Options::default() }
    }

    #[test]
    fn interleaved_blocks_are_joined_per_taxon() {
        let out = convert(INTERLEAVED, &no_wrap()).unwrap();
        assert_eq!(
            out,
            ">Alpha\nACGTACGTACGT\n>Beta\nACGAACGTACGT\n>Gamma\nTCGAACGTACGT\n"
        );
    }

    #[test]
    fn sequential_relaxed_long_names_survive() {
        let out = convert(SEQUENTIAL, &no_wrap()).unwrap();
        assert_eq!(out, ">Homo_sapiens\nACGTACGT\n>Pan_troglodytes\nACGTTCGT\n");
    }

    #[test]
    fn sequential_wrapped_lines_are_concatenated() {
        let input = "2 12\nAlpha     ACGT\nACGTACGT\nBeta      TTTT\nTTTTTTTT\n";
        let out = convert(input, &no_wrap()).unwrap();
        assert_eq!(out, ">Alpha\nACGTACGTACGT\n>Beta\nTTTTTTTTTTTT\n");
    }

    #[test]
    fn strict_names_without_a_separator_are_split_at_column_ten() {
        // "Salmonella" is exactly 10 characters and butts against the data, so
        // only the strict split yields 8 sites — auto must fall back to it.
        let input = "2 8\nSalmonellaACGTACGT\nEscherichiACGTTCGT\n";
        let out = convert(input, &no_wrap()).unwrap();
        assert_eq!(out, ">Salmonella\nACGTACGT\n>Escherichi\nACGTTCGT\n");
    }

    #[test]
    fn interleaved_flag_in_the_header_is_honoured() {
        let input = "2 8 I\nAlpha     ACGT\nBeta      TTTT\nACGT\nTTTT\n";
        let out = convert(input, &no_wrap()).unwrap();
        assert_eq!(out, ">Alpha\nACGTACGT\n>Beta\nTTTTTTTT\n");
    }

    #[test]
    fn interleaved_continuation_blocks_may_repeat_the_name() {
        let input = "2 8\nAlpha     ACGT\nBeta      TTTT\n\nAlpha     ACGT\nBeta      TTTT\n";
        let out = convert(input, &no_wrap()).unwrap();
        assert_eq!(out, ">Alpha\nACGTACGT\n>Beta\nTTTTTTTT\n");
    }

    #[test]
    fn gaps_are_preserved_by_default() {
        let input = "1 8\nAlpha     AC--GT..\n";
        let out = convert(input, &no_wrap()).unwrap();
        assert_eq!(out, ">Alpha\nAC--GT..\n");
    }

    #[test]
    fn remove_gaps_produces_unaligned_fasta() {
        let input = "1 8\nAlpha     AC--GT..\n";
        let opts = Options { remove_gaps: true, ..no_wrap() };
        assert_eq!(convert(input, &opts).unwrap(), ">Alpha\nACGT\n");
    }

    #[test]
    fn uppercase_normalizes_residues() {
        let input = "1 8\nAlpha     acgtacgt\n";
        let opts = Options { uppercase: true, ..no_wrap() };
        assert_eq!(convert(input, &opts).unwrap(), ">Alpha\nACGTACGT\n");
    }

    #[test]
    fn default_wrap_is_sixty_characters() {
        let seq: String = "AC".repeat(35); // 70 sites
        let input = format!("1 70\nAlpha     {seq}\n");
        let out = convert(&input, &Options::default()).unwrap();
        let expected = format!(">Alpha\n{}\n{}\n", &seq[..60], &seq[60..]);
        assert_eq!(out, expected);
    }

    #[test]
    fn wrap_zero_emits_one_line_per_sequence() {
        let out = convert(SEQUENTIAL, &no_wrap()).unwrap();
        assert_eq!(out.lines().count(), 4);
    }

    #[test]
    fn wrap_is_capped_at_max_wrap() {
        let opts = Options { wrap: 999_999, ..Options::default() };
        let out = convert(SEQUENTIAL, &opts).unwrap();
        assert_eq!(out, ">Homo_sapiens\nACGTACGT\n>Pan_troglodytes\nACGTTCGT\n");
    }

    #[test]
    fn forced_sequential_layout_rejects_an_interleaved_file() {
        // Read sequentially, the second block's lines get glued onto the wrong
        // taxa, so the site counts stop matching the header.
        let opts = Options { layout: Layout::Sequential, ..no_wrap() };
        let err = convert(INTERLEAVED, &opts).unwrap_err();
        assert!(err.contains("the header declares 12"), "got: {err}");
    }

    #[test]
    fn sequential_layout_reports_leftover_lines() {
        // Two taxa of 4 sites each, but three data lines: the third is unread.
        let opts = Options { layout: Layout::Sequential, ..no_wrap() };
        let err = convert("2 4\nAlpha     ACGT\nBeta      TTTT\nGamma     GGGG\n", &opts)
            .unwrap_err();
        assert!(err.contains("unread line"), "got: {err}");
    }

    #[test]
    fn forced_interleaved_layout_parses_the_interleaved_file() {
        let opts = Options { layout: Layout::Interleaved, ..no_wrap() };
        let out = convert(INTERLEAVED, &opts).unwrap();
        assert!(out.starts_with(">Alpha\nACGTACGTACGT\n"), "got: {out}");
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(convert("", &Options::default()).unwrap(), "");
        assert_eq!(convert("\n\n", &Options::default()).unwrap(), "");
    }

    #[test]
    fn rejects_missing_count_header() {
        let err = convert(">Alpha\nACGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("count header"), "got: {err}");
    }

    #[test]
    fn rejects_non_numeric_site_count() {
        let err = convert("3 twelve\nAlpha ACGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("site (alignment length) count"), "got: {err}");
    }

    #[test]
    fn rejects_site_count_mismatch() {
        let err = convert("1 12\nAlpha     ACGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("site(s) but the header declares 12"), "got: {err}");
    }

    #[test]
    fn rejects_taxon_count_mismatch() {
        let err = convert("3 8\nAlpha     ACGTACGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("declares 3 taxa"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_residue_characters() {
        let err = convert("1 8\nAlpha     ACGT/CGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("invalid residue character"), "got: {err}");
    }

    #[test]
    fn tolerant_accepts_a_site_count_mismatch() {
        let opts = Options { tolerant: true, ..no_wrap() };
        let out = convert("1 12\nAlpha     ACGT\n", &opts).unwrap();
        assert_eq!(out, ">Alpha\nACGT\n");
    }

    #[test]
    fn tolerant_accepts_invalid_residue_characters() {
        let opts = Options { tolerant: true, ..no_wrap() };
        let out = convert("1 8\nAlpha     ACGT/CGT\n", &opts).unwrap();
        assert_eq!(out, ">Alpha\nACGT/CGT\n");
    }

    #[test]
    fn forced_strict_name_style_truncates_long_names_at_ten_columns() {
        let opts = Options { name_style: NameStyle::Strict, tolerant: true, ..no_wrap() };
        let out = convert("1 8\nHomo_sapiens ACGTACGT\n", &opts).unwrap();
        assert_eq!(out, ">Homo_sapie\nnsACGTACGT\n");
    }

    #[test]
    fn parse_exposes_records_for_reuse() {
        let records = parse(SEQUENTIAL, &Options::default()).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].name, "Homo_sapiens");
        assert_eq!(records[1].sequence, "ACGTTCGT");
    }
}
