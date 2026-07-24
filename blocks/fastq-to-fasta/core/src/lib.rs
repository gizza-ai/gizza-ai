//! fastq-to-fasta core — strip per-base quality lines from FASTQ reads and emit
//! clean FASTA, with optional length + mean-quality filtering, line wrapping,
//! header renaming, N-read discarding, and uppercasing. Pure compute; no
//! wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

/// Largest line-wrap width `wrap` may request. `wrap = 0` disables wrapping
/// (one sequence line per read). Bounds the descriptor so the LLM-/CLI-facing
/// schema can't drift from what `convert` enforces.
pub const MAX_WRAP: u32 = 1000;

/// Options controlling the FASTQ → FASTA conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Drop reads shorter than this many bases. `0` = no length filter.
    pub min_length: usize,
    /// Drop reads whose MEAN Phred quality is below this. `0` = no quality filter.
    pub min_quality: f64,
    /// Phred ASCII offset: `33` (Sanger / Illumina 1.8+) or `64` (old Illumina 1.3–1.5).
    pub quality_offset: u8,
    /// Wrap sequence lines at this many characters. `0` = one line per sequence.
    pub wrap: usize,
    /// Replace each header with a sequential number (`>1`, `>2`, …).
    pub rename: bool,
    /// Discard reads that contain an ambiguous `N`/`n` base.
    pub strip_n: bool,
    /// Uppercase the sequence bases.
    pub uppercase: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            min_length: 0,
            min_quality: 0.0,
            quality_offset: 33,
            wrap: 0,
            rename: false,
            strip_n: false,
            uppercase: false,
        }
    }
}

/// Convert FASTQ text to FASTA text.
///
/// Each FASTQ read is four lines: `@id [description]`, the sequence, a `+` line
/// (optionally repeating the id), and the per-base quality string (same length as
/// the sequence). This drops the `+` and quality lines and rewrites `@` → `>`.
///
/// Returns `Err` on malformed FASTQ (record count not a multiple of 4, a header
/// not starting with `@`, a separator not starting with `+`, a
/// sequence/quality length mismatch, or a quality char below the chosen offset),
/// or on an invalid `quality_offset`.
pub fn convert(input: &str, opts: &Options) -> Result<String, String> {
    if opts.quality_offset != 33 && opts.quality_offset != 64 {
        return Err(format!(
            "invalid quality_offset {}: expected 33 (Sanger/Illumina 1.8+) or 64 (old Illumina)",
            opts.quality_offset
        ));
    }
    let wrap = (opts.wrap as u32).min(MAX_WRAP) as usize;

    // Normalize line endings (CRLF/CR → LF) and split, dropping only trailing
    // blank lines (a final newline, or a couple of them, is common and harmless).
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = normalized.split('\n').collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    if lines.is_empty() {
        return Ok(String::new());
    }
    if lines.len() % 4 != 0 {
        return Err(format!(
            "malformed FASTQ: expected a multiple of 4 lines (4 per read), got {}",
            lines.len()
        ));
    }

    let mut out = String::new();
    let mut kept: usize = 0;
    for (i, record) in lines.chunks(4).enumerate() {
        let read_no = i + 1;
        let header = record[0];
        let sequence = record[1];
        let separator = record[2];
        let quality = record[3];

        if !header.starts_with('@') {
            return Err(format!(
                "malformed FASTQ: read {read_no} header must start with '@', found {header:?}"
            ));
        }
        if !separator.starts_with('+') {
            return Err(format!(
                "malformed FASTQ: read {read_no} separator (line 3) must start with '+', found {separator:?}"
            ));
        }
        if sequence.len() != quality.len() {
            return Err(format!(
                "malformed FASTQ: read {read_no} sequence length ({}) != quality length ({})",
                sequence.len(),
                quality.len()
            ));
        }

        // Length filter (bases).
        if sequence.chars().count() < opts.min_length {
            continue;
        }
        // Discard reads containing ambiguous N.
        if opts.strip_n && sequence.bytes().any(|b| b == b'N' || b == b'n') {
            continue;
        }
        // Mean-quality filter (only when requested and the read is non-empty).
        if opts.min_quality > 0.0 && !quality.is_empty() {
            let mut sum: u64 = 0;
            for b in quality.bytes() {
                if b < opts.quality_offset {
                    return Err(format!(
                        "malformed FASTQ: read {read_no} has a quality character (byte {b}) below the offset {}; wrong quality_offset?",
                        opts.quality_offset
                    ));
                }
                sum += (b - opts.quality_offset) as u64;
            }
            let mean = sum as f64 / quality.len() as f64;
            if mean < opts.min_quality {
                continue;
            }
        }

        kept += 1;
        // Header: '>' + either the original id/description or a sequential number.
        out.push('>');
        if opts.rename {
            out.push_str(&kept.to_string());
        } else {
            out.push_str(&header[1..]);
        }
        out.push('\n');

        // Sequence, optionally uppercased and wrapped.
        let seq_owned;
        let seq: &str = if opts.uppercase {
            seq_owned = sequence.to_ascii_uppercase();
            &seq_owned
        } else {
            sequence
        };
        if wrap == 0 {
            out.push_str(seq);
            out.push('\n');
        } else {
            let bytes = seq.as_bytes();
            let mut start = 0;
            while start < bytes.len() {
                let end = (start + wrap).min(bytes.len());
                // Safe: FASTA sequences are ASCII bases, so byte slicing == char slicing.
                out.push_str(&seq[start..end]);
                out.push('\n');
                start = end;
            }
            // An empty sequence still gets one (blank) line so it round-trips.
            if bytes.is_empty() {
                out.push('\n');
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_READS: &str = "@read1 sample\nACGTACGTNN\n+\nIIIIIIII##\n@read2\nACGT\n+\n!!!!";

    #[test]
    fn basic_conversion_strips_quality_and_keeps_headers() {
        let out = convert(TWO_READS, &Options::default()).unwrap();
        assert_eq!(out, ">read1 sample\nACGTACGTNN\n>read2\nACGT\n");
    }

    #[test]
    fn single_read_with_trailing_newline() {
        let out = convert("@r\nACGT\n+\nIIII\n", &Options::default()).unwrap();
        assert_eq!(out, ">r\nACGT\n");
    }

    #[test]
    fn min_length_drops_short_reads() {
        let opts = Options { min_length: 5, ..Options::default() };
        let out = convert(TWO_READS, &opts).unwrap();
        // read2 (ACGT, 4 bases) is dropped; read1 (10 bases) survives.
        assert_eq!(out, ">read1 sample\nACGTACGTNN\n");
    }

    #[test]
    fn min_quality_drops_low_quality_reads() {
        // read2 quality '!!!!' = Phred 0; read1 mean is high. Offset 33.
        let opts = Options { min_quality: 20.0, quality_offset: 33, ..Options::default() };
        let out = convert(TWO_READS, &opts).unwrap();
        assert_eq!(out, ">read1 sample\nACGTACGTNN\n");
    }

    #[test]
    fn strip_n_discards_reads_with_n() {
        let opts = Options { strip_n: true, ..Options::default() };
        let out = convert(TWO_READS, &opts).unwrap();
        // read1 contains NN → dropped; read2 clean → kept.
        assert_eq!(out, ">read2\nACGT\n");
    }

    #[test]
    fn rename_replaces_headers_with_sequential_numbers() {
        let opts = Options { rename: true, ..Options::default() };
        let out = convert(TWO_READS, &opts).unwrap();
        assert_eq!(out, ">1\nACGTACGTNN\n>2\nACGT\n");
    }

    #[test]
    fn uppercase_normalizes_bases() {
        let opts = Options { uppercase: true, ..Options::default() };
        let out = convert("@r\nacgtN\n+\nIIIII\n", &opts).unwrap();
        assert_eq!(out, ">r\nACGTN\n");
    }

    #[test]
    fn wrap_splits_long_sequences() {
        let opts = Options { wrap: 4, ..Options::default() };
        let out = convert("@r\nACGTACGTAC\n+\nIIIIIIIIII\n", &opts).unwrap();
        assert_eq!(out, ">r\nACGT\nACGT\nAC\n");
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(convert("", &Options::default()).unwrap(), "");
        assert_eq!(convert("\n\n", &Options::default()).unwrap(), "");
    }

    #[test]
    fn rejects_non_multiple_of_four_lines() {
        let err = convert("@r\nACGT\n+", &Options::default()).unwrap_err();
        assert!(err.contains("multiple of 4"), "got: {err}");
    }

    #[test]
    fn rejects_bad_header() {
        let err = convert(">r\nACGT\n+\nIIII", &Options::default()).unwrap_err();
        assert!(err.contains("must start with '@'"), "got: {err}");
    }

    #[test]
    fn rejects_bad_separator() {
        let err = convert("@r\nACGT\n-\nIIII", &Options::default()).unwrap_err();
        assert!(err.contains("must start with '+'"), "got: {err}");
    }

    #[test]
    fn rejects_seq_quality_length_mismatch() {
        let err = convert("@r\nACGT\n+\nII", &Options::default()).unwrap_err();
        assert!(err.contains("!= quality length"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_quality_offset() {
        let opts = Options { quality_offset: 42, ..Options::default() };
        let err = convert(TWO_READS, &opts).unwrap_err();
        assert!(err.contains("invalid quality_offset"), "got: {err}");
    }

    #[test]
    fn quality_char_below_offset_errors_when_filtering() {
        // '!' = byte 33; offset 64 → below offset → error (wrong offset picked).
        let opts = Options { min_quality: 10.0, quality_offset: 64, ..Options::default() };
        let err = convert("@r\nACGT\n+\n!!!!", &opts).unwrap_err();
        assert!(err.contains("below the offset"), "got: {err}");
    }

    #[test]
    fn wrap_is_capped_at_max_wrap() {
        // A wrap far above MAX_WRAP behaves like MAX_WRAP (single line for short seq).
        let opts = Options { wrap: 999_999, ..Options::default() };
        let out = convert("@r\nACGT\n+\nIIII\n", &opts).unwrap();
        assert_eq!(out, ">r\nACGT\n");
    }
}
