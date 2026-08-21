//! fasta-to-csv core — parse FASTA text into a delimited table (CSV/TSV) of
//! `id`, `description`, `sequence` and `length`, with optional GC-content and
//! per-base-count columns, uppercasing and sequence deduplication.
//!
//! Pure compute; no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.

/// Largest number of FASTA records a single conversion will parse. Bounds the
/// browser tab's memory use and gives the descriptor a documented cap.
pub const MAX_RECORDS: usize = 50_000;

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Comma,
    Tab,
    Semicolon,
    Pipe,
}

impl Delimiter {
    /// The literal character written between fields.
    pub fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Tab => '\t',
            Delimiter::Semicolon => ';',
            Delimiter::Pipe => '|',
        }
    }

    /// Parse the descriptor's enum value (blank → the `comma` default).
    pub fn parse(s: &str) -> Result<Delimiter, String> {
        match s.trim() {
            "" | "comma" => Ok(Delimiter::Comma),
            "tab" => Ok(Delimiter::Tab),
            "semicolon" => Ok(Delimiter::Semicolon),
            "pipe" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter {other:?}: expected \"comma\", \"tab\", \"semicolon\" or \"pipe\""
            )),
        }
    }
}

/// How the `>` header line becomes column(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMode {
    /// `id` = text up to the first whitespace, `description` = the rest.
    Split,
    /// `id` = text up to the first whitespace; the rest is discarded.
    IdOnly,
    /// `id` = the whole header line (minus the `>`); no description column.
    FullHeader,
}

impl HeaderMode {
    /// Parse the descriptor's enum value (blank → the `split` default).
    pub fn parse(s: &str) -> Result<HeaderMode, String> {
        match s.trim() {
            "" | "split" => Ok(HeaderMode::Split),
            "id_only" => Ok(HeaderMode::IdOnly),
            "full_header" => Ok(HeaderMode::FullHeader),
            other => Err(format!(
                "invalid header_mode {other:?}: expected \"split\", \"id_only\" or \"full_header\""
            )),
        }
    }
}

/// Options controlling the FASTA → CSV conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Field separator for the output table.
    pub delimiter: Delimiter,
    /// How the `>` header becomes `id` (+ `description`).
    pub header_mode: HeaderMode,
    /// Emit a header row naming each column.
    pub header_row: bool,
    /// Emit the `sequence` column.
    pub include_sequence: bool,
    /// Emit the `length` column (character count of the joined sequence).
    pub include_length: bool,
    /// Emit the `gc_percent` column — `(G+C)/(A+C+G+T) * 100`, 2 decimals.
    pub include_gc: bool,
    /// Emit the `a_count`/`c_count`/`g_count`/`t_count`/`other_count` columns.
    pub include_base_counts: bool,
    /// Uppercase the emitted sequence (`acgt` → `ACGT`).
    pub uppercase: bool,
    /// Drop records whose sequence (case-insensitively) already appeared.
    pub dedupe: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            delimiter: Delimiter::Comma,
            header_mode: HeaderMode::Split,
            header_row: true,
            include_sequence: true,
            include_length: true,
            include_gc: false,
            include_base_counts: false,
            uppercase: false,
            dedupe: false,
        }
    }
}

/// One parsed FASTA record.
struct Record {
    id: String,
    description: String,
    sequence: String,
}

/// Quote a field per RFC 4180: wrap in `"` and double any embedded `"` when the
/// value contains the delimiter, a quote, or a line break.
fn quote(field: &str, delim: char) -> String {
    if field.contains(delim) || field.contains('"') || field.contains('\n') || field.contains('\r')
    {
        let mut out = String::with_capacity(field.len() + 2);
        out.push('"');
        for c in field.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
        out
    } else {
        field.to_string()
    }
}

/// Split a header (already stripped of `>`) into id + description at the first
/// run of whitespace.
fn split_header(header: &str) -> (String, String) {
    match header.find(char::is_whitespace) {
        Some(i) => (header[..i].to_string(), header[i..].trim_start().to_string()),
        None => (header.to_string(), String::new()),
    }
}

/// Parse FASTA text into records, joining wrapped sequence lines.
///
/// Blank lines are ignored. Any non-blank line before the first `>` header is an
/// error, as is an input with no `>` header at all.
fn parse(input: &str) -> Result<Vec<Record>, String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut records: Vec<Record> = Vec::new();

    for (idx, raw) in normalized.split('\n').enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('>') {
            if records.len() == MAX_RECORDS {
                return Err(format!(
                    "too many records: this tool converts at most {MAX_RECORDS} FASTA records per run; split the file and convert it in parts"
                ));
            }
            let header = header.trim();
            let (id, description) = split_header(header);
            records.push(Record { id, description, sequence: String::new() });
        } else if let Some(rec) = records.last_mut() {
            rec.sequence.push_str(line);
        } else {
            return Err(format!(
                "malformed FASTA: line {} is sequence data ({:?}) but no '>' header line came before it",
                idx + 1,
                line.chars().take(20).collect::<String>()
            ));
        }
    }

    if records.is_empty() {
        return Err(
            "no FASTA records found: the input must contain at least one '>' header line followed by its sequence"
                .to_string(),
        );
    }
    Ok(records)
}

/// GC percentage over unambiguous bases only: `(G+C) / (A+C+G+T) * 100`.
/// Returns `0.0` when the record has no A/C/G/T at all.
fn gc_percent(seq: &str) -> f64 {
    let (mut gc, mut acgt) = (0u64, 0u64);
    for b in seq.bytes() {
        match b.to_ascii_uppercase() {
            b'G' | b'C' => {
                gc += 1;
                acgt += 1;
            }
            b'A' | b'T' | b'U' => acgt += 1,
            _ => {}
        }
    }
    if acgt == 0 {
        0.0
    } else {
        gc as f64 * 100.0 / acgt as f64
    }
}

/// Case-insensitive A/C/G/T counts plus everything else (N, gaps, amino acids…).
fn base_counts(seq: &str) -> [u64; 5] {
    let mut counts = [0u64; 5];
    for c in seq.chars() {
        match c.to_ascii_uppercase() {
            'A' => counts[0] += 1,
            'C' => counts[1] += 1,
            'G' => counts[2] += 1,
            'T' => counts[3] += 1,
            _ => counts[4] += 1,
        }
    }
    counts
}

/// The header-row labels for the enabled columns, in output order.
pub fn column_names(opts: &Options) -> Vec<&'static str> {
    let mut cols = vec!["id"];
    if opts.header_mode == HeaderMode::Split {
        cols.push("description");
    }
    if opts.include_sequence {
        cols.push("sequence");
    }
    if opts.include_length {
        cols.push("length");
    }
    if opts.include_gc {
        cols.push("gc_percent");
    }
    if opts.include_base_counts {
        cols.extend_from_slice(&["a_count", "c_count", "g_count", "t_count", "other_count"]);
    }
    cols
}

/// Convert FASTA text to a delimited table.
///
/// Wrapped sequence lines are joined into one field, blank lines are ignored,
/// and every field is RFC-4180 quoted when it contains the delimiter, a quote or
/// a line break. The output always ends with a newline.
///
/// Returns `Err` when the input has no `>` header, when sequence data appears
/// before the first header, or when the record cap is exceeded.
pub fn convert(input: &str, opts: &Options) -> Result<String, String> {
    let records = parse(input)?;
    let delim = opts.delimiter.ch();

    let mut out = String::new();
    if opts.header_row {
        let names = column_names(opts);
        for (i, name) in names.iter().enumerate() {
            if i > 0 {
                out.push(delim);
            }
            out.push_str(&quote(name, delim));
        }
        out.push('\n');
    }

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for rec in &records {
        if opts.dedupe && !seen.insert(rec.sequence.to_ascii_uppercase()) {
            continue;
        }

        let seq_owned;
        let seq: &str = if opts.uppercase {
            seq_owned = rec.sequence.to_ascii_uppercase();
            &seq_owned
        } else {
            &rec.sequence
        };

        let mut fields: Vec<String> = Vec::new();
        match opts.header_mode {
            HeaderMode::Split => {
                fields.push(rec.id.clone());
                fields.push(rec.description.clone());
            }
            HeaderMode::IdOnly => fields.push(rec.id.clone()),
            HeaderMode::FullHeader => {
                let full = if rec.description.is_empty() {
                    rec.id.clone()
                } else {
                    format!("{} {}", rec.id, rec.description)
                };
                fields.push(full);
            }
        }
        if opts.include_sequence {
            fields.push(seq.to_string());
        }
        if opts.include_length {
            fields.push(rec.sequence.chars().count().to_string());
        }
        if opts.include_gc {
            fields.push(format!("{:.2}", gc_percent(&rec.sequence)));
        }
        if opts.include_base_counts {
            for n in base_counts(&rec.sequence) {
                fields.push(n.to_string());
            }
        }

        for (i, f) in fields.iter().enumerate() {
            if i > 0 {
                out.push(delim);
            }
            out.push_str(&quote(f, delim));
        }
        out.push('\n');
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = ">seq1 first sequence\nACGTACGTNN\n>seq2\nacgt\n";

    #[test]
    fn default_emits_id_description_sequence_length_with_header_row() {
        let out = convert(TWO, &Options::default()).unwrap();
        assert_eq!(
            out,
            "id,description,sequence,length\n\
             seq1,first sequence,ACGTACGTNN,10\n\
             seq2,,acgt,4\n"
        );
    }

    #[test]
    fn wrapped_sequence_lines_are_joined() {
        let out = convert(">s\nACGT\nACGT\n\nAC\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\ns,,ACGTACGTAC,10\n");
    }

    #[test]
    fn header_row_can_be_turned_off() {
        let opts = Options { header_row: false, ..Options::default() };
        let out = convert(">s desc\nACGT\n", &opts).unwrap();
        assert_eq!(out, "s,desc,ACGT,4\n");
    }

    #[test]
    fn tab_delimiter_emits_tsv() {
        let opts = Options { delimiter: Delimiter::Tab, ..Options::default() };
        let out = convert(">s desc\nACGT\n", &opts).unwrap();
        assert_eq!(out, "id\tdescription\tsequence\tlength\ns\tdesc\tACGT\t4\n");
    }

    #[test]
    fn semicolon_and_pipe_delimiters() {
        let semi = Options { delimiter: Delimiter::Semicolon, ..Options::default() };
        assert_eq!(convert(">s d\nAC\n", &semi).unwrap(), "id;description;sequence;length\ns;d;AC;2\n");
        let pipe = Options { delimiter: Delimiter::Pipe, ..Options::default() };
        assert_eq!(convert(">s d\nAC\n", &pipe).unwrap(), "id|description|sequence|length\ns|d|AC|2\n");
    }

    #[test]
    fn description_containing_the_delimiter_is_quoted() {
        let out = convert(">s alpha, beta\nAC\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\ns,\"alpha, beta\",AC,2\n");
    }

    #[test]
    fn embedded_quotes_are_doubled() {
        let out = convert(">s say \"hi\", ok\nAC\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\ns,\"say \"\"hi\"\", ok\",AC,2\n");
    }

    #[test]
    fn tab_delimiter_leaves_commas_unquoted() {
        let opts = Options { delimiter: Delimiter::Tab, ..Options::default() };
        let out = convert(">s alpha, beta\nAC\n", &opts).unwrap();
        assert_eq!(out, "id\tdescription\tsequence\tlength\ns\talpha, beta\tAC\t2\n");
    }

    #[test]
    fn id_only_drops_the_description_column() {
        let opts = Options { header_mode: HeaderMode::IdOnly, ..Options::default() };
        let out = convert(TWO, &opts).unwrap();
        assert_eq!(out, "id,sequence,length\nseq1,ACGTACGTNN,10\nseq2,acgt,4\n");
    }

    #[test]
    fn full_header_keeps_the_whole_header_in_one_column() {
        let opts = Options { header_mode: HeaderMode::FullHeader, ..Options::default() };
        let out = convert(TWO, &opts).unwrap();
        assert_eq!(out, "id,sequence,length\nseq1 first sequence,ACGTACGTNN,10\nseq2,acgt,4\n");
        // A header carrying the delimiter is quoted as one cell.
        let out = convert(">gi|1|ref|NM_1.1 Homo sapiens, mRNA\nAC\n", &opts).unwrap();
        assert_eq!(
            out,
            "id,sequence,length\n\"gi|1|ref|NM_1.1 Homo sapiens, mRNA\",AC,2\n"
        );
    }

    #[test]
    fn sequence_column_can_be_dropped() {
        let opts = Options { include_sequence: false, ..Options::default() };
        let out = convert(TWO, &opts).unwrap();
        assert_eq!(out, "id,description,length\nseq1,first sequence,10\nseq2,,4\n");
    }

    #[test]
    fn length_column_can_be_dropped() {
        let opts = Options { include_length: false, ..Options::default() };
        let out = convert(">s d\nACGT\n", &opts).unwrap();
        assert_eq!(out, "id,description,sequence\ns,d,ACGT\n");
    }

    #[test]
    fn gc_percent_uses_unambiguous_bases_only() {
        let opts = Options {
            include_gc: true,
            include_sequence: false,
            include_length: false,
            header_mode: HeaderMode::IdOnly,
            ..Options::default()
        };
        // GGCC + NN → 4/4 unambiguous bases are G/C → 100.00.
        assert_eq!(convert(">s\nGGCCNN\n", &opts).unwrap(), "id,gc_percent\ns,100.00\n");
        // ACGT → 2 of 4 → 50.00; lowercase counts the same.
        assert_eq!(convert(">s\nacgt\n", &opts).unwrap(), "id,gc_percent\ns,50.00\n");
        // No A/C/G/T at all → 0.00 rather than a divide-by-zero.
        assert_eq!(convert(">s\nNNNN\n", &opts).unwrap(), "id,gc_percent\ns,0.00\n");
    }

    #[test]
    fn base_counts_bucket_everything_else_into_other() {
        let opts = Options {
            include_base_counts: true,
            include_sequence: false,
            include_length: false,
            header_mode: HeaderMode::IdOnly,
            ..Options::default()
        };
        let out = convert(">s\nAACGTnN-\n", &opts).unwrap();
        assert_eq!(
            out,
            "id,a_count,c_count,g_count,t_count,other_count\ns,2,1,1,1,3\n"
        );
    }

    #[test]
    fn uppercase_normalizes_the_sequence_column_only() {
        let opts = Options { uppercase: true, ..Options::default() };
        let out = convert(">s desc\nacgtn\n", &opts).unwrap();
        assert_eq!(out, "id,description,sequence,length\ns,desc,ACGTN,5\n");
    }

    #[test]
    fn dedupe_keeps_the_first_of_each_duplicate_sequence() {
        let opts = Options { dedupe: true, ..Options::default() };
        let out = convert(">a\nACGT\n>b\nacgt\n>c\nTTTT\n", &opts).unwrap();
        assert_eq!(out, "id,description,sequence,length\na,,ACGT,4\nc,,TTTT,4\n");
    }

    #[test]
    fn crlf_input_is_normalized() {
        let out = convert(">s d\r\nACGT\r\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\ns,d,ACGT,4\n");
    }

    #[test]
    fn header_with_no_sequence_yields_an_empty_sequence_field() {
        let out = convert(">empty\n>s\nAC\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\nempty,,,0\ns,,AC,2\n");
    }

    #[test]
    fn bare_gt_header_gives_an_empty_id() {
        let out = convert(">\nAC\n", &Options::default()).unwrap();
        assert_eq!(out, "id,description,sequence,length\n,,AC,2\n");
    }

    #[test]
    fn rejects_input_with_no_header() {
        let err = convert("ACGT\n", &Options::default()).unwrap_err();
        assert!(err.contains("no '>' header line came before it"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = convert("   \n\n", &Options::default()).unwrap_err();
        assert!(err.contains("no FASTA records found"), "got: {err}");
    }

    #[test]
    fn rejects_bad_delimiter_and_header_mode_values() {
        assert!(Delimiter::parse("colon").unwrap_err().contains("invalid delimiter"));
        assert!(HeaderMode::parse("whole").unwrap_err().contains("invalid header_mode"));
        assert_eq!(Delimiter::parse("").unwrap(), Delimiter::Comma);
        assert_eq!(HeaderMode::parse("").unwrap(), HeaderMode::Split);
    }

    #[test]
    fn record_cap_boundary_passes_at_max_and_fails_one_over() {
        let at_cap: String = (0..MAX_RECORDS).map(|i| format!(">s{i}\nAC\n")).collect();
        let opts = Options { header_row: false, ..Options::default() };
        let out = convert(&at_cap, &opts).unwrap();
        assert_eq!(out.lines().count(), MAX_RECORDS);

        let over = format!("{at_cap}>one_too_many\nAC\n");
        let err = convert(&over, &opts).unwrap_err();
        assert!(err.contains("too many records"), "got: {err}");
    }
}
