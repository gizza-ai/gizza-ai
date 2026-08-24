//! dna-reverse-complement core — pure compute, shared by the chat skill block and the web page.
//!
//! Turns a DNA/RNA sequence into its reverse complement (or just its complement,
//! or just its reverse). The full IUPAC nucleotide alphabet is supported, and the
//! degenerate pairs follow the IUPAC convention:
//!
//! ```text
//! A<->T   C<->G   U->A    R<->Y   K<->M   B<->V   D<->H
//! S->S    W->W    N->N    gaps (- .) map to themselves
//! ```
//!
//! `S` (G|C) and `W` (A|T) are self-complementary — a classic error source in
//! hand-rolled tables, so they are unit-tested explicitly.
//!
//! Input may be raw sequence or (multi-record) FASTA; whitespace between bases is
//! always ignored, headers are preserved, and each record is transformed on its own.
//! No I/O, no allocations beyond the output — safe in every surface (chat, CLI, page).

/// Maximum number of characters accepted in the `sequence` input. A browser tab is
/// not a compute cluster; 1,000,000 characters is roughly a bacterial chromosome.
pub const MAX_INPUT_CHARS: usize = 1_000_000;

/// Maximum accepted output line width (`0` means "do not wrap").
pub const MAX_LINE_WIDTH: usize = 200;

/// Which transform to apply to each record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Complement every base, then reverse the sequence (the default).
    ReverseComplement,
    /// Complement every base, keeping the 5'->3' order as typed.
    Complement,
    /// Reverse the order of the bases without complementing them.
    Reverse,
}

/// Which nucleotide alphabet the OUTPUT should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alphabet {
    /// Keep the input's alphabet: RNA out if the input has `U` and no `T`, else DNA.
    Auto,
    /// Force DNA output (`U` becomes `T`).
    Dna,
    /// Force RNA output (`T` becomes `U`).
    Rna,
}

/// What to do with a character that is not a nucleotide, IUPAC code, or gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnInvalid {
    /// Reject the input and say which character was wrong and where (the default).
    Error,
    /// Silently remove the character.
    Drop,
    /// Pass the character through untouched (it is still repositioned by a reverse).
    Keep,
}

/// Everything the transform needs besides the sequence itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub operation: Operation,
    pub output_alphabet: Alphabet,
    /// Keep the input's upper/lower case (lower case is often used to mark regions).
    pub preserve_case: bool,
    /// Wrap output sequence lines at this many characters; `0` = one line per record.
    pub line_width: usize,
    pub on_invalid: OnInvalid,
    /// Append a `#`-prefixed composition summary after the sequence.
    pub show_stats: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            operation: Operation::ReverseComplement,
            output_alphabet: Alphabet::Auto,
            preserve_case: true,
            line_width: 0,
            on_invalid: OnInvalid::Error,
            show_stats: false,
        }
    }
}

/// Parse the `operation` parameter. An empty string selects the default.
pub fn parse_operation(s: &str) -> Result<Operation, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "reverse_complement" => Ok(Operation::ReverseComplement),
        "complement" => Ok(Operation::Complement),
        "reverse" => Ok(Operation::Reverse),
        other => Err(format!(
            "invalid operation {other:?}: expected \"reverse_complement\", \"complement\", or \"reverse\""
        )),
    }
}

/// Parse the `output_alphabet` parameter. An empty string selects the default.
pub fn parse_alphabet(s: &str) -> Result<Alphabet, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Alphabet::Auto),
        "dna" => Ok(Alphabet::Dna),
        "rna" => Ok(Alphabet::Rna),
        other => Err(format!(
            "invalid output_alphabet {other:?}: expected \"auto\", \"dna\", or \"rna\""
        )),
    }
}

/// Parse the `on_invalid` parameter. An empty string selects the default.
pub fn parse_on_invalid(s: &str) -> Result<OnInvalid, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "error" => Ok(OnInvalid::Error),
        "drop" => Ok(OnInvalid::Drop),
        "keep" => Ok(OnInvalid::Keep),
        other => Err(format!(
            "invalid on_invalid {other:?}: expected \"error\", \"drop\", or \"keep\""
        )),
    }
}

/// True for any character this tool recognises as part of a sequence: the four
/// DNA bases, RNA's `U`, the eleven IUPAC ambiguity codes, and the two gap symbols.
pub fn is_sequence_char(c: char) -> bool {
    matches!(
        c.to_ascii_uppercase(),
        'A' | 'C' | 'G' | 'T' | 'U' | 'R' | 'Y' | 'S' | 'W' | 'K' | 'M' | 'B' | 'D' | 'H' | 'V'
            | 'N' | '-' | '.'
    )
}

/// IUPAC complement of one recognised character, preserving its case. Characters
/// this table does not know (only reachable under [`OnInvalid::Keep`]) come back
/// unchanged.
fn complement_char(c: char) -> char {
    let upper = c.to_ascii_uppercase();
    let out = match upper {
        'A' => 'T',
        'C' => 'G',
        'G' => 'C',
        'T' => 'A',
        'U' => 'A',
        'R' => 'Y', // A|G  -> C|T
        'Y' => 'R', // C|T  -> A|G
        'S' => 'S', // G|C  -> C|G, self-complementary
        'W' => 'W', // A|T  -> T|A, self-complementary
        'K' => 'M', // G|T  -> C|A
        'M' => 'K', // A|C  -> T|G
        'B' => 'V', // C|G|T -> G|C|A
        'V' => 'B', // A|C|G -> T|G|C
        'D' => 'H', // A|G|T -> T|C|A
        'H' => 'D', // A|C|T -> T|G|A
        'N' => 'N',
        other => other, // gaps and (with on_invalid=keep) opaque characters
    };
    if c.is_ascii_lowercase() {
        out.to_ascii_lowercase()
    } else {
        out
    }
}

/// Re-letter one character into the requested alphabet (`U`<->`T`), keeping case.
fn transliterate(c: char, alphabet: Alphabet) -> char {
    match (alphabet, c) {
        (Alphabet::Dna, 'U') => 'T',
        (Alphabet::Dna, 'u') => 't',
        (Alphabet::Rna, 'T') => 'U',
        (Alphabet::Rna, 't') => 'u',
        _ => c,
    }
}

/// One FASTA record (or the single unheaded record of a raw-sequence input).
struct Record {
    /// The header line INCLUDING its leading `>`, or `None` for raw input.
    header: Option<String>,
    /// Sequence characters with whitespace already removed.
    chars: Vec<char>,
}

/// Split the input into records. A line starting with `>` anywhere in the input
/// switches on FASTA mode; sequence appearing before the first header becomes an
/// unheaded leading record so nothing is ever silently dropped.
fn split_records(input: &str) -> Vec<Record> {
    let fasta = input.lines().any(|l| l.starts_with('>'));
    if !fasta {
        return vec![Record {
            header: None,
            chars: input.chars().filter(|c| !c.is_whitespace()).collect(),
        }];
    }
    let mut records: Vec<Record> = Vec::new();
    for line in input.lines() {
        if let Some(stripped) = line.strip_prefix('>') {
            records.push(Record {
                header: Some(format!(">{}", stripped.trim_end())),
                chars: Vec::new(),
            });
        } else {
            if records.is_empty() {
                if line.trim().is_empty() {
                    continue;
                }
                records.push(Record {
                    header: None,
                    chars: Vec::new(),
                });
            }
            let last = records.last_mut().expect("record pushed above");
            last.chars.extend(line.chars().filter(|c| !c.is_whitespace()));
        }
    }
    records
}

/// Composition counts of the produced sequence, used by `show_stats`.
#[derive(Default)]
struct Stats {
    records: usize,
    bases: usize,
    gc: usize,
    unambiguous: usize,
    ambiguous: usize,
    gaps: usize,
}

impl Stats {
    fn tally(&mut self, c: char) {
        match c.to_ascii_uppercase() {
            '-' | '.' => self.gaps += 1,
            'A' | 'C' | 'G' | 'T' | 'U' => {
                self.bases += 1;
                self.unambiguous += 1;
                if matches!(c.to_ascii_uppercase(), 'G' | 'C') {
                    self.gc += 1;
                }
            }
            'R' | 'Y' | 'S' | 'W' | 'K' | 'M' | 'B' | 'D' | 'H' | 'V' | 'N' => {
                self.bases += 1;
                self.ambiguous += 1;
            }
            _ => self.bases += 1, // opaque characters kept via on_invalid=keep
        }
    }

    fn render(&self) -> String {
        let gc = if self.unambiguous == 0 {
            "n/a (no unambiguous bases)".to_string()
        } else {
            format!(
                "{:.2}%",
                (self.gc as f64) * 100.0 / (self.unambiguous as f64)
            )
        };
        format!(
            "# sequences: {}\n# length: {}\n# gc_content: {}\n# ambiguous: {}\n# gaps: {}",
            self.records, self.bases, gc, self.ambiguous, self.gaps
        )
    }
}

/// Wrap `chars` into lines of `width` characters (`0` = a single line).
fn wrap(chars: &[char], width: usize) -> String {
    if width == 0 || chars.len() <= width {
        return chars.iter().collect();
    }
    let mut out = String::with_capacity(chars.len() + chars.len() / width + 1);
    for (i, chunk) in chars.chunks(width).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.extend(chunk.iter());
    }
    out
}

/// Reverse-complement (or complement / reverse) a DNA or RNA sequence.
///
/// Accepts raw sequence or multi-record FASTA. Returns the transformed sequence
/// as text, or a human-readable message explaining exactly what was rejected.
pub fn convert(sequence: &str, opts: &Options) -> Result<String, String> {
    let len = sequence.chars().count();
    if len > MAX_INPUT_CHARS {
        return Err(format!(
            "input is {len} characters; the maximum is {MAX_INPUT_CHARS}"
        ));
    }
    if opts.line_width > MAX_LINE_WIDTH {
        return Err(format!(
            "line_width is {}; the maximum is {MAX_LINE_WIDTH} (use 0 for one line per sequence)",
            opts.line_width
        ));
    }

    let records = split_records(sequence);

    // Pass 1 — validate and filter, so an invalid character is reported before any
    // partial output exists, and so `auto` can see the whole input's alphabet.
    let mut cleaned: Vec<(Option<String>, Vec<char>)> = Vec::with_capacity(records.len());
    let mut saw_u = false;
    let mut saw_t = false;
    for record in &records {
        let mut kept: Vec<char> = Vec::with_capacity(record.chars.len());
        for (idx, &c) in record.chars.iter().enumerate() {
            if !is_sequence_char(c) {
                match opts.on_invalid {
                    OnInvalid::Error => {
                        let where_ = match &record.header {
                            Some(h) => format!(" of record {h:?}"),
                            None => String::new(),
                        };
                        return Err(format!(
                            "invalid sequence character {c:?} at position {}{where_}: expected A/C/G/T/U, an IUPAC ambiguity code (R Y S W K M B D H V N), or a gap (- .). Set on_invalid to \"drop\" to remove such characters or \"keep\" to pass them through.",
                            idx + 1
                        ));
                    }
                    OnInvalid::Drop => continue,
                    OnInvalid::Keep => {}
                }
            }
            match c.to_ascii_uppercase() {
                'U' => saw_u = true,
                'T' => saw_t = true,
                _ => {}
            }
            kept.push(c);
        }
        cleaned.push((record.header.clone(), kept));
    }

    if cleaned.iter().all(|(_, chars)| chars.is_empty()) {
        return Err("no sequence found: paste DNA or RNA bases (raw or FASTA)".to_string());
    }

    let alphabet = match opts.output_alphabet {
        Alphabet::Auto => {
            if saw_u && !saw_t {
                Alphabet::Rna
            } else {
                Alphabet::Dna
            }
        }
        forced => forced,
    };

    // Pass 2 — transform.
    let mut stats = Stats::default();
    let mut blocks: Vec<String> = Vec::with_capacity(cleaned.len());
    for (header, chars) in &cleaned {
        let mut out: Vec<char> = chars
            .iter()
            .map(|&c| match opts.operation {
                Operation::Reverse => c,
                Operation::Complement | Operation::ReverseComplement => complement_char(c),
            })
            .map(|c| transliterate(c, alphabet))
            .map(|c| {
                if opts.preserve_case {
                    c
                } else {
                    c.to_ascii_uppercase()
                }
            })
            .collect();
        if matches!(
            opts.operation,
            Operation::Reverse | Operation::ReverseComplement
        ) {
            out.reverse();
        }

        stats.records += 1;
        for &c in &out {
            stats.tally(c);
        }

        let body = wrap(&out, opts.line_width);
        match header {
            Some(h) => blocks.push(format!("{h}\n{body}")),
            None => blocks.push(body),
        }
    }

    let mut result = blocks.join("\n");
    if opts.show_stats {
        result.push_str("\n\n");
        result.push_str(&stats.render());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn reverse_complements_a_plain_dna_sequence() {
        assert_eq!(convert("ATGC", &opts()).unwrap(), "GCAT");
        assert_eq!(convert("AAAACCCGGT", &opts()).unwrap(), "ACCGGGTTTT");
    }

    #[test]
    fn rejects_an_unknown_character_with_its_position() {
        let err = convert("ACGTXACGT", &opts()).unwrap_err();
        assert!(err.contains("'X'"), "{err}");
        assert!(err.contains("position 5"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert("   \n  ", &opts()).unwrap_err();
        assert!(err.contains("no sequence found"), "{err}");
    }

    #[test]
    fn iupac_degenerate_pairs_follow_the_convention() {
        // Complement only, so each code sits opposite its own input position.
        let o = Options {
            operation: Operation::Complement,
            ..opts()
        };
        assert_eq!(convert("RYKMBVDHNSW", &o).unwrap(), "YRMKVBHDNSW");
    }

    #[test]
    fn s_and_w_are_self_complementary() {
        let o = Options {
            operation: Operation::Complement,
            ..opts()
        };
        assert_eq!(convert("SSWW", &o).unwrap(), "SSWW");
    }

    #[test]
    fn reverse_only_does_not_complement() {
        let o = Options {
            operation: Operation::Reverse,
            ..opts()
        };
        assert_eq!(convert("ATGC", &o).unwrap(), "CGTA");
    }

    #[test]
    fn case_is_preserved_by_default_and_foldable() {
        assert_eq!(convert("atGC", &opts()).unwrap(), "GCat");
        let o = Options {
            preserve_case: false,
            ..opts()
        };
        assert_eq!(convert("atGC", &o).unwrap(), "GCAT");
    }

    #[test]
    fn auto_alphabet_keeps_rna_and_can_be_forced() {
        // U present, no T -> RNA output.
        assert_eq!(convert("AUGC", &opts()).unwrap(), "GCAU");
        // Forced DNA re-letters U as T.
        let dna = Options {
            output_alphabet: Alphabet::Dna,
            ..opts()
        };
        assert_eq!(convert("AUGC", &dna).unwrap(), "GCAT");
        // Forced RNA re-letters T as U.
        let rna = Options {
            output_alphabet: Alphabet::Rna,
            ..opts()
        };
        assert_eq!(convert("ATGC", &rna).unwrap(), "GCAU");
    }

    #[test]
    fn mixed_t_and_u_defaults_to_dna() {
        assert_eq!(convert("ATGU", &opts()).unwrap(), "ACAT");
    }

    #[test]
    fn whitespace_between_bases_is_ignored() {
        assert_eq!(convert("AT GC\nAT\tGC", &opts()).unwrap(), "GCATGCAT");
    }

    #[test]
    fn fasta_records_are_transformed_individually_with_headers_kept() {
        let out = convert(">one desc\nATGC\n>two\nAAAA\n", &opts()).unwrap();
        assert_eq!(out, ">one desc\nGCAT\n>two\nTTTT");
    }

    #[test]
    fn fasta_sequence_lines_are_joined_before_transforming() {
        let out = convert(">r\nAT\nGC\n", &opts()).unwrap();
        assert_eq!(out, ">r\nGCAT");
    }

    #[test]
    fn line_width_wraps_the_output() {
        let o = Options {
            line_width: 4,
            ..opts()
        };
        let out = convert("AAAACCCCGG", &o).unwrap();
        assert_eq!(out, "CCGG\nGGTT\nTT");
    }

    #[test]
    fn line_width_above_the_cap_is_rejected() {
        let o = Options {
            line_width: MAX_LINE_WIDTH + 1,
            ..opts()
        };
        let err = convert("ACGT", &o).unwrap_err();
        assert!(err.contains("maximum is 200"), "{err}");
    }

    #[test]
    fn gaps_map_to_themselves() {
        assert_eq!(convert("AC-GT", &opts()).unwrap(), "AC-GT");
        assert_eq!(convert("A.CG", &opts()).unwrap(), "CG.T");
    }

    #[test]
    fn on_invalid_drop_removes_junk_and_keep_passes_it_through() {
        let drop = Options {
            on_invalid: OnInvalid::Drop,
            ..opts()
        };
        assert_eq!(convert("1 ACGT 10", &drop).unwrap(), "ACGT");
        let keep = Options {
            on_invalid: OnInvalid::Keep,
            ..opts()
        };
        assert_eq!(convert("ACGT*", &keep).unwrap(), "*ACGT");
    }

    #[test]
    fn stats_report_length_and_gc() {
        let o = Options {
            show_stats: true,
            ..opts()
        };
        let out = convert("ACGTNN--", &o).unwrap();
        assert!(out.starts_with("--NNACGT\n\n"), "{out}");
        assert!(out.contains("# sequences: 1"), "{out}");
        assert!(out.contains("# length: 6"), "{out}");
        assert!(out.contains("# gc_content: 50.00%"), "{out}");
        assert!(out.contains("# ambiguous: 2"), "{out}");
        assert!(out.contains("# gaps: 2"), "{out}");
    }

    #[test]
    fn input_above_the_character_cap_is_rejected() {
        let big = "A".repeat(MAX_INPUT_CHARS + 1);
        let err = convert(&big, &opts()).unwrap_err();
        assert!(err.contains("maximum is 1000000"), "{err}");
    }

    #[test]
    fn cap_boundary_is_accepted() {
        let at_cap = "A".repeat(MAX_INPUT_CHARS);
        let out = convert(&at_cap, &opts()).unwrap();
        assert_eq!(out.len(), MAX_INPUT_CHARS);
        assert!(out.chars().all(|c| c == 'T'));
    }

    #[test]
    fn parsers_accept_the_advertised_values_and_reject_others() {
        assert_eq!(parse_operation("").unwrap(), Operation::ReverseComplement);
        assert_eq!(parse_operation("complement").unwrap(), Operation::Complement);
        assert_eq!(parse_operation("reverse").unwrap(), Operation::Reverse);
        assert!(parse_operation("revcomp").is_err());
        assert_eq!(parse_alphabet("").unwrap(), Alphabet::Auto);
        assert_eq!(parse_alphabet("RNA").unwrap(), Alphabet::Rna);
        assert!(parse_alphabet("protein").is_err());
        assert_eq!(parse_on_invalid("").unwrap(), OnInvalid::Error);
        assert_eq!(parse_on_invalid("drop").unwrap(), OnInvalid::Drop);
        assert!(parse_on_invalid("ignore").is_err());
    }

    /// Reverse-complementing twice returns the original (DNA, upper case).
    #[test]
    fn double_reverse_complement_round_trips() {
        let seq = "ACGTRYKMBVDHNSW";
        let once = convert(seq, &opts()).unwrap();
        let twice = convert(&once, &opts()).unwrap();
        assert_eq!(twice, seq);
    }
}
