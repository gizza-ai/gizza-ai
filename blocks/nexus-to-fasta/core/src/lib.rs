//! nexus-to-fasta core — extract the sequence matrix from a NEXUS file's
//! `DATA` (or `CHARACTERS`) block and emit standard FASTA.
//!
//! Pure compute; no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.
//!
//! A NEXUS file opens with the `#NEXUS` token and is then a series of blocks:
//!
//! ```text
//! #NEXUS
//! [ a bracketed comment ]
//! begin data;
//!   dimensions ntax=2 nchar=8;
//!   format datatype=dna missing=? gap=- matchchar=.;
//!   matrix
//!     Alpha  ACGTACGT
//!     Beta   ..G..T..
//!   ;
//! end;
//! ```
//!
//! The pieces that matter for a FASTA conversion are `dimensions` (the declared
//! taxon and site counts), `format` (the gap / missing / matchchar symbols and
//! the `interleave` flag) and `matrix` (the data itself). The matrix comes in
//! two layouts — sequential (each taxon's whole sequence before the next) and
//! interleaved (repeated blocks holding one chunk per taxon) — and both are
//! auto-detected by checking each candidate parse against the declared `nchar`.

/// Largest line-wrap width `wrap` may request. `wrap = 0` disables wrapping
/// (one sequence line per taxon). Bounds the descriptor so the LLM-/CLI-facing
/// schema can't drift from what `convert` enforces.
pub const MAX_WRAP: u32 = 1000;

/// How the NEXUS matrix is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// Honour the `interleave` flag, else try both and keep the parse whose
    /// sequence lengths match the declared `nchar`.
    Auto,
    /// Each taxon's whole sequence appears before the next taxon's.
    Sequential,
    /// The matrix is split into blocks; every block holds one chunk per taxon.
    Interleaved,
}

/// What to do with the case of the sequence residues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Emit the residues exactly as they appear in the matrix.
    Keep,
    /// Uppercase every residue (`acgt` → `ACGT`).
    Upper,
    /// Lowercase every residue (`ACGT` → `acgt`).
    Lower,
}

/// Options controlling the NEXUS → FASTA conversion.
#[derive(Debug, Clone)]
pub struct Options {
    /// Sequential vs interleaved matrix layout.
    pub layout: Layout,
    /// Wrap sequence lines at this many characters. `0` = one line per sequence.
    pub wrap: usize,
    /// Residue case normalisation.
    pub case: Case,
    /// Strip the alignment gap characters (the declared `gap=` symbol plus the
    /// conventional `-` and `.`), turning the aligned FASTA into unaligned
    /// sequences.
    pub remove_gaps: bool,
    /// Replace the declared `matchchar` symbol with the residue the first taxon
    /// carries at that site.
    pub expand_matchchar: bool,
    /// Turn `_` in an UNQUOTED taxon label into a space, the NEXUS convention.
    /// Quoted labels are always taken literally.
    pub underscores_to_spaces: bool,
    /// Accept a file whose taxon count or sequence lengths disagree with its
    /// `dimensions` command instead of reporting an error.
    pub tolerant: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            layout: Layout::Auto,
            wrap: 60,
            case: Case::Keep,
            remove_gaps: false,
            expand_matchchar: true,
            underscores_to_spaces: false,
            tolerant: false,
        }
    }
}

/// One parsed taxon: its label and its sequence, split into per-site units.
///
/// A unit is normally a single residue character, but a `standard`-datatype
/// matrix may write one site as a bracketed state set — `(01)` for a
/// polymorphism, `{01}` for an ambiguity — which counts as ONE site against
/// `nchar` and must never be split by line wrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub sites: Vec<String>,
}

impl Record {
    /// The sequence as one flat string.
    pub fn sequence(&self) -> String {
        self.sites.concat()
    }
}

/// The `format` command's symbol declarations, with the NEXUS defaults applied.
#[derive(Debug, Clone)]
struct FormatSpec {
    /// The `gap=` symbol; NEXUS has no default, but `-` is universal.
    gap: char,
    /// The `matchchar=` symbol, if one was declared.
    matchchar: Option<char>,
    /// True when the `format` command carries `interleave` / `interleave=yes`.
    interleave: bool,
    /// True when `labels=no` / `nolabels` says the matrix rows carry no taxon
    /// labels — the names then come from the `TAXA` block's `taxlabels`.
    labels: bool,
}

impl Default for FormatSpec {
    fn default() -> Self {
        FormatSpec { gap: '-', matchchar: None, interleave: false, labels: true }
    }
}

/// Strip NEXUS `[...]` comments, honouring nesting and single-quoted labels.
///
/// Comments are replaced by a single space so that `Alpha[note]ACGT` cannot
/// silently glue a label onto its sequence. Returns an error on an unterminated
/// comment or quote, which is nearly always a truncated file.
fn strip_comments(src: &str) -> Result<String, String> {
    let mut out = String::with_capacity(src.len());
    let mut depth = 0usize;
    let mut in_quote = false;
    let mut chars = src.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quote {
            out.push(c);
            if c == '\'' {
                // A doubled '' inside a quoted label is an escaped apostrophe.
                if chars.peek() == Some(&'\'') {
                    out.push(chars.next().unwrap());
                } else {
                    in_quote = false;
                }
            }
            continue;
        }
        match c {
            '\'' if depth == 0 => {
                in_quote = true;
                out.push(c);
            }
            '[' => depth += 1,
            ']' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    "not NEXUS: found a comment terminator ']' with no matching '['".to_string()
                })?;
                out.push(' ');
            }
            // Keep newlines inside comments so line numbers/blocks stay aligned.
            '\n' if depth > 0 => out.push('\n'),
            _ if depth > 0 => {}
            _ => out.push(c),
        }
    }
    if in_quote {
        return Err("not NEXUS: a single-quoted taxon label is never closed".to_string());
    }
    if depth > 0 {
        return Err("not NEXUS: a '[' comment is never closed with ']'".to_string());
    }
    Ok(out)
}

/// Find the body of the first `begin <name>;` … `end;` block, case-insensitively.
///
/// `names` is tried in order, so callers can prefer `DATA` over `CHARACTERS`.
/// Returns the text between the `begin` command's `;` and the block's `end`.
fn find_block<'a>(src: &'a str, names: &[&str]) -> Option<&'a str> {
    let lower = src.to_ascii_lowercase();
    for name in names {
        let mut from = 0usize;
        while let Some(rel) = lower[from..].find("begin") {
            let start = from + rel;
            from = start + 5;
            // `begin` must be its own word.
            if start > 0 && lower.as_bytes()[start - 1].is_ascii_alphanumeric() {
                continue;
            }
            let after = &lower[start + 5..];
            let trimmed = after.trim_start();
            if !trimmed.starts_with(name) {
                continue;
            }
            let rest = &trimmed[name.len()..];
            if !rest.starts_with(|c: char| c.is_whitespace() || c == ';') {
                continue;
            }
            // Absolute offset of the `;` that ends the `begin …;` command.
            let semi = start + 5 + (after.len() - trimmed.len()) + name.len() + rest.find(';')?;
            let body_start = semi + 1;
            let end_rel = find_block_end(&lower[body_start..])?;
            return Some(&src[body_start..body_start + end_rel]);
        }
    }
    None
}

/// Offset of the `end;` / `endblock;` that closes a block body (lowercased input).
fn find_block_end(body: &str) -> Option<usize> {
    let mut from = 0usize;
    while let Some(rel) = body[from..].find("end") {
        let at = from + rel;
        from = at + 3;
        if at > 0 && body.as_bytes()[at - 1].is_ascii_alphanumeric() {
            continue;
        }
        let rest = &body[at + 3..];
        let rest = rest.strip_prefix("block").unwrap_or(rest);
        if rest.trim_start().starts_with(';') {
            return Some(at);
        }
    }
    None
}

/// Split a block body into `(command_name_lowercased, argument_text)` pairs.
///
/// Commands are terminated by `;`, except `matrix`, whose argument runs to the
/// block's own terminating `;` and may contain none of its own.
fn split_commands(body: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = body;
    while !rest.trim().is_empty() {
        let Some(semi) = rest.find(';') else {
            // A final unterminated command (a matrix missing its `;`).
            let chunk = rest.trim();
            if !chunk.is_empty() {
                out.push(split_command_word(chunk));
            }
            break;
        };
        let chunk = rest[..semi].trim();
        if !chunk.is_empty() {
            out.push(split_command_word(chunk));
        }
        rest = &rest[semi + 1..];
    }
    out
}

/// Split one command into its leading keyword (lowercased) and its arguments.
fn split_command_word(chunk: &str) -> (String, String) {
    match chunk.find(char::is_whitespace) {
        Some(i) => (chunk[..i].to_ascii_lowercase(), chunk[i..].trim().to_string()),
        None => (chunk.to_ascii_lowercase(), String::new()),
    }
}

/// Parse `dimensions ntax=… nchar=…` into `(ntax, nchar)`; either may be absent.
fn parse_dimensions(args: &str) -> (Option<usize>, Option<usize>) {
    let mut ntax = None;
    let mut nchar = None;
    for (key, value) in key_values(args) {
        match key.as_str() {
            "ntax" | "ntaxa" => ntax = value.parse().ok(),
            "nchar" | "nchars" => nchar = value.parse().ok(),
            _ => {}
        }
    }
    (ntax, nchar)
}

/// Parse the `format` command's subcommands into a [`FormatSpec`].
fn parse_format(args: &str) -> FormatSpec {
    let mut spec = FormatSpec::default();
    for (key, value) in key_values(args) {
        match key.as_str() {
            "gap" => {
                if let Some(c) = value.chars().next() {
                    spec.gap = c;
                }
            }
            "matchchar" => spec.matchchar = value.chars().next(),
            // `interleave` is a bare flag in most writers, `interleave=yes` in some.
            "interleave" => spec.interleave = !matches!(value.as_str(), "no" | "false"),
            "nointerleave" => spec.interleave = false,
            "labels" => spec.labels = !matches!(value.as_str(), "no" | "false"),
            "nolabels" => spec.labels = false,
            _ => {}
        }
    }
    spec
}

/// Tokenise `key=value` / bare-flag subcommands, honouring quotes.
///
/// A bare flag yields an empty value, so `interleave` and `interleave=yes` are
/// both seen as the key `interleave`.
fn key_values(args: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut key = String::new();
    let mut value = String::new();
    let mut in_value = false;
    let mut quote: Option<char> = None;
    for c in args.chars() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if in_value {
                value.push(c);
            } else {
                key.push(c);
            }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '=' => in_value = true,
            c if c.is_whitespace() => {
                if !key.is_empty() {
                    out.push((key.to_ascii_lowercase(), value.to_ascii_lowercase()));
                }
                key.clear();
                value.clear();
                in_value = false;
            }
            c if in_value => value.push(c),
            c => key.push(c),
        }
    }
    if !key.is_empty() {
        out.push((key.to_ascii_lowercase(), value.to_ascii_lowercase()));
    }
    out
}

/// Read a taxon label from the front of `line`, returning `(label, rest)`.
///
/// A label is either a single-quoted string (where `''` is a literal apostrophe
/// and `_` stays an underscore) or the first whitespace-delimited word (where
/// `_` optionally becomes a space, the NEXUS convention for unquoted labels).
fn take_label(line: &str, underscores_to_spaces: bool) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(body) = trimmed.strip_prefix('\'') {
        let mut label = String::new();
        let mut chars = body.char_indices();
        while let Some((i, c)) = chars.next() {
            if c != '\'' {
                label.push(c);
                continue;
            }
            if body[i + 1..].starts_with('\'') {
                label.push('\'');
                chars.next();
                continue;
            }
            return Some((label, &body[i + 1..]));
        }
        // Unterminated quote — `strip_comments` already rejects that case.
        return Some((label, ""));
    }
    let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let raw = &trimmed[..end];
    let label = if underscores_to_spaces { raw.replace('_', " ") } else { raw.to_string() };
    Some((label, &trimmed[end..]))
}

/// Split a run of sequence text into per-site units.
///
/// Whitespace is dropped (NEXUS lets writers space out codons); a `(...)` or
/// `{...}` state set is kept whole as a single site.
fn split_sites(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() {
            continue;
        }
        let close = match c {
            '(' => Some(')'),
            '{' => Some('}'),
            _ => None,
        };
        match close {
            Some(close) => {
                let mut unit = String::from(c);
                for c in chars.by_ref() {
                    unit.push(c);
                    if c == close {
                        break;
                    }
                }
                out.push(unit);
            }
            None => out.push(c.to_string()),
        }
    }
    out
}

/// Parse a sequential matrix: a label, then residues until `nchar` sites have
/// been read, then the next label.
///
/// `nchar` is required here — without it there is no way to know where one
/// taxon's (possibly line-wrapped) sequence ends and the next label begins.
fn parse_sequential(matrix: &str, nchar: Option<usize>, opts: &Options) -> Result<Vec<Record>, String> {
    let nchar = nchar.ok_or_else(|| {
        "cannot read a sequential matrix without a site count: add 'dimensions nchar=<n>;' to the block, or set the layout to interleaved".to_string()
    })?;
    let mut records: Vec<Record> = Vec::new();
    let mut rest = matrix;
    loop {
        let Some((name, after)) = take_label(rest, opts.underscores_to_spaces) else {
            break;
        };
        let mut sites: Vec<String> = Vec::new();
        rest = after;
        // Consume whitespace-separated residue runs until the taxon is full.
        while sites.len() < nchar {
            let trimmed = rest.trim_start();
            if trimmed.is_empty() {
                break;
            }
            let end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            let mut chunk = split_sites(&trimmed[..end]);
            let room = nchar - sites.len();
            if chunk.len() > room {
                // The token overshoots: keep the fill and leave the tail for the
                // length check to report (a mis-declared nchar).
                chunk.truncate(room);
                sites.extend(chunk);
                rest = &trimmed[end..];
                break;
            }
            sites.extend(chunk);
            rest = &trimmed[end..];
        }
        records.push(Record { name, sites });
    }
    Ok(records)
}

/// Parse an interleaved matrix: one labelled row per taxon per block, with the
/// taxa repeating in the same order in every block.
fn parse_interleaved(matrix: &str, opts: &Options) -> Result<Vec<Record>, String> {
    let mut records: Vec<Record> = Vec::new();
    for line in matrix.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((name, rest)) = take_label(line, opts.underscores_to_spaces) else {
            continue;
        };
        let sites = split_sites(rest);
        match records.iter_mut().find(|r| r.name == name) {
            Some(existing) => existing.sites.extend(sites),
            None => records.push(Record { name, sites }),
        }
    }
    Ok(records)
}

/// Parse a matrix whose rows carry no labels (`format labels=no`), pairing each
/// row with the corresponding name from the `TAXA` block in order.
fn parse_unlabelled(matrix: &str, taxa: &[String], nchar: Option<usize>) -> Result<Vec<Record>, String> {
    if taxa.is_empty() {
        return Err(
            "the format command says labels=no, but no taxon names were found: add a 'begin taxa; … taxlabels …;' block, or remove labels=no"
                .to_string(),
        );
    }
    let mut records: Vec<Record> =
        taxa.iter().map(|n| Record { name: n.clone(), sites: Vec::new() }).collect();
    let mut index = 0usize;
    for line in matrix.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let sites = split_sites(line);
        if sites.is_empty() {
            continue;
        }
        let record = &mut records[index % taxa.len()];
        record.sites.extend(sites);
        // With a known nchar, a row is only finished once it is full; without
        // one, every line is simply the next taxon's row.
        match nchar {
            Some(n) if record.sites.len() < n => {}
            _ => index += 1,
        }
    }
    Ok(records)
}

/// Read the taxon names out of a `TAXA` block's `taxlabels` command.
fn parse_taxlabels(src: &str, underscores_to_spaces: bool) -> Vec<String> {
    let Some(body) = find_block(src, &["taxa"]) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for (name, args) in split_commands(body) {
        if name != "taxlabels" {
            continue;
        }
        let mut rest = args.as_str();
        while let Some((label, after)) = take_label(rest, underscores_to_spaces) {
            names.push(label);
            rest = after;
        }
    }
    names
}

/// Replace every `matchchar` site with the residue the FIRST taxon carries at
/// the same position — the NEXUS shorthand for "identical to the reference".
fn expand_matchchar(records: &mut [Record], matchchar: char) {
    let Some(reference) = records.first().map(|r| r.sites.clone()) else {
        return;
    };
    let symbol = matchchar.to_string();
    for record in records.iter_mut().skip(1) {
        for (i, site) in record.sites.iter_mut().enumerate() {
            if *site == symbol {
                if let Some(replacement) = reference.get(i) {
                    *site = replacement.clone();
                }
            }
        }
    }
}

/// Check a parse against the declared dimensions.
fn validate(records: &[Record], ntax: Option<usize>, nchar: Option<usize>) -> Result<(), String> {
    if records.is_empty() {
        return Err("the matrix command is empty: no taxon rows were found".to_string());
    }
    if let Some(ntax) = ntax {
        if records.len() != ntax {
            return Err(format!(
                "the dimensions command declares {ntax} taxa but the matrix holds {}",
                records.len()
            ));
        }
    }
    if let Some(nchar) = nchar {
        for record in records {
            if record.sites.len() != nchar {
                return Err(format!(
                    "taxon {:?} has {} sites but the dimensions command declares nchar={nchar}",
                    record.name,
                    record.sites.len()
                ));
            }
        }
    }
    Ok(())
}

/// Wrap a sequence at `width` sites per line (`0` = one line), never splitting a
/// bracketed state set.
fn wrap_sites(sites: &[String], width: usize) -> String {
    if width == 0 {
        return sites.concat();
    }
    let mut out = String::new();
    for chunk in sites.chunks(width) {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&chunk.concat());
    }
    out
}

/// Parse the records out of a NEXUS document, applying `opts.layout`.
///
/// Exposed so callers can inspect the matrix without rendering FASTA.
pub fn parse(nexus: &str, opts: &Options) -> Result<Vec<Record>, String> {
    if nexus.trim().is_empty() {
        return Err("no input: paste a NEXUS file (it starts with '#NEXUS')".to_string());
    }
    let src = strip_comments(nexus)?;
    let body = find_block(&src, &["data", "characters"]).ok_or_else(|| {
        "not NEXUS: no 'begin data;' or 'begin characters;' block was found (the matrix lives in one of those, closed by 'end;')"
            .to_string()
    })?;

    let mut ntax = None;
    let mut nchar = None;
    let mut spec = FormatSpec::default();
    let mut matrix: Option<String> = None;
    for (name, args) in split_commands(body) {
        match name.as_str() {
            "dimensions" => {
                let (t, c) = parse_dimensions(&args);
                ntax = ntax.or(t);
                nchar = nchar.or(c);
            }
            "format" => spec = parse_format(&args),
            "matrix" => matrix = Some(args),
            _ => {}
        }
    }
    let matrix = matrix.ok_or_else(|| {
        "the data block has no 'matrix' command: that is where the sequences live".to_string()
    })?;

    let mut records = if !spec.labels {
        let taxa = parse_taxlabels(&src, opts.underscores_to_spaces);
        parse_unlabelled(&matrix, &taxa, nchar)?
    } else {
        match opts.layout {
            Layout::Sequential => parse_sequential(&matrix, nchar, opts)?,
            Layout::Interleaved => parse_interleaved(&matrix, opts)?,
            Layout::Auto => {
                // The declared flag wins; otherwise keep whichever candidate
                // parse agrees with the declared dimensions.
                let (first, second) = if spec.interleave {
                    (parse_interleaved(&matrix, opts), parse_sequential(&matrix, nchar, opts))
                } else {
                    (parse_sequential(&matrix, nchar, opts), parse_interleaved(&matrix, opts))
                };
                match first {
                    Ok(records) if validate(&records, ntax, nchar).is_ok() => records,
                    first => match second {
                        Ok(records) if validate(&records, ntax, nchar).is_ok() => records,
                        _ => first?,
                    },
                }
            }
        }
    };

    if let Some(matchchar) = spec.matchchar {
        if opts.expand_matchchar {
            expand_matchchar(&mut records, matchchar);
        }
    }
    if !opts.tolerant {
        validate(&records, ntax, nchar)?;
    } else if records.is_empty() {
        return Err("the matrix command is empty: no taxon rows were found".to_string());
    }

    if opts.remove_gaps {
        let gap = spec.gap.to_string();
        for record in records.iter_mut() {
            record.sites.retain(|s| s != &gap && s != "-" && s != ".");
        }
    }
    match opts.case {
        Case::Keep => {}
        Case::Upper => records
            .iter_mut()
            .for_each(|r| r.sites.iter_mut().for_each(|s| *s = s.to_uppercase())),
        Case::Lower => records
            .iter_mut()
            .for_each(|r| r.sites.iter_mut().for_each(|s| *s = s.to_lowercase())),
    }
    Ok(records)
}

/// Convert NEXUS alignment text into FASTA.
///
/// Returns the FASTA document (one `>name` header plus the wrapped sequence per
/// taxon, trailing newline included) or a human-readable error naming what went
/// wrong and where.
pub fn convert(nexus: &str, opts: &Options) -> Result<String, String> {
    if opts.wrap > MAX_WRAP as usize {
        return Err(format!(
            "wrap must be between 0 and {MAX_WRAP} characters per line, got {}",
            opts.wrap
        ));
    }
    let records = parse(nexus, opts)?;
    let mut out = String::new();
    for record in &records {
        out.push('>');
        out.push_str(&record.name);
        out.push('\n');
        out.push_str(&wrap_sites(&record.sites, opts.wrap));
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEQUENTIAL: &str = "#NEXUS\nbegin data;\n  dimensions ntax=2 nchar=8;\n  format datatype=dna missing=? gap=-;\n  matrix\n    Alpha ACGTACGT\n    Beta  ACGTTCGT\n  ;\nend;\n";

    const INTERLEAVED: &str = "#NEXUS\nbegin data;\n  dimensions ntax=2 nchar=8;\n  format datatype=dna gap=- interleave;\n  matrix\n    Alpha ACGT\n    Beta  ACGT\n\n    Alpha ACGT\n    Beta  TCGT\n  ;\nend;\n";

    fn flat() -> Options {
        Options { wrap: 0, ..Options::default() }
    }

    #[test]
    fn converts_a_sequential_matrix() {
        assert_eq!(
            convert(SEQUENTIAL, &flat()).unwrap(),
            ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n"
        );
    }

    #[test]
    fn converts_an_interleaved_matrix() {
        assert_eq!(
            convert(INTERLEAVED, &flat()).unwrap(),
            ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n"
        );
    }

    #[test]
    fn auto_detects_interleaved_without_the_flag() {
        let src = INTERLEAVED.replace(" interleave", "");
        assert_eq!(convert(&src, &flat()).unwrap(), ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n");
    }

    #[test]
    fn strips_bracketed_comments_including_nested_ones() {
        let src = "#NEXUS\n[ a note [ nested ] still a note ]\nbegin data;\n dimensions ntax=1 nchar=4;\n matrix Alpha[inline]ACGT ;\nend;\n";
        assert_eq!(convert(src, &flat()).unwrap(), ">Alpha\nACGT\n");
    }

    #[test]
    fn expands_matchchar_against_the_first_taxon() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=8;\n format datatype=dna gap=- matchchar=.;\n matrix\n  Alpha ACGTACGT\n  Beta  ....T...\n ;\nend;\n";
        assert_eq!(convert(src, &flat()).unwrap(), ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n");
    }

    #[test]
    fn keeps_matchchar_verbatim_when_expansion_is_off() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=8;\n format datatype=dna matchchar=.;\n matrix\n  Alpha ACGTACGT\n  Beta  ....T...\n ;\nend;\n";
        let opts = Options { expand_matchchar: false, ..flat() };
        assert_eq!(convert(src, &opts).unwrap(), ">Alpha\nACGTACGT\n>Beta\n....T...\n");
    }

    #[test]
    fn reads_quoted_labels_and_underscore_names() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=4;\n matrix\n  'Ginkgo biloba' ACGT\n  Homo_sapiens ACGA\n ;\nend;\n";
        let opts = Options { underscores_to_spaces: true, ..flat() };
        assert_eq!(
            convert(src, &opts).unwrap(),
            ">Ginkgo biloba\nACGT\n>Homo sapiens\nACGA\n"
        );
        // Left off, the underscore survives as an underscore.
        assert!(convert(src, &flat()).unwrap().contains(">Homo_sapiens"));
    }

    #[test]
    fn reads_a_characters_block_too() {
        let src = SEQUENTIAL.replace("begin data;", "begin characters;");
        assert_eq!(convert(&src, &flat()).unwrap(), ">Alpha\nACGTACGT\n>Beta\nACGTTCGT\n");
    }

    #[test]
    fn uses_taxa_labels_when_the_matrix_has_none() {
        let src = "#NEXUS\nbegin taxa;\n dimensions ntax=2;\n taxlabels Alpha Beta;\nend;\nbegin data;\n dimensions ntax=2 nchar=4;\n format datatype=dna labels=no;\n matrix\n  ACGT\n  TCGA\n ;\nend;\n";
        assert_eq!(convert(src, &flat()).unwrap(), ">Alpha\nACGT\n>Beta\nTCGA\n");
    }

    #[test]
    fn keeps_state_sets_as_one_site() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=1 nchar=4;\n format datatype=standard symbols=\"01\";\n matrix Alpha 01(01)1 ;\nend;\n";
        let opts = Options { wrap: 2, ..Options::default() };
        assert_eq!(convert(src, &opts).unwrap(), ">Alpha\n01\n(01)1\n");
    }

    #[test]
    fn wraps_at_the_requested_width() {
        let opts = Options { wrap: 4, ..Options::default() };
        assert_eq!(
            convert(SEQUENTIAL, &opts).unwrap(),
            ">Alpha\nACGT\nACGT\n>Beta\nACGT\nTCGT\n"
        );
    }

    #[test]
    fn removes_gaps_and_normalises_case() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=6;\n format datatype=dna gap=-;\n matrix\n  Alpha ac--gt\n  Beta  a-cg-t\n ;\nend;\n";
        let opts = Options { remove_gaps: true, case: Case::Upper, ..flat() };
        assert_eq!(convert(src, &opts).unwrap(), ">Alpha\nACGT\n>Beta\nACGT\n");
    }

    #[test]
    fn lowercases_on_request() {
        let opts = Options { case: Case::Lower, ..flat() };
        assert_eq!(convert(SEQUENTIAL, &opts).unwrap(), ">Alpha\nacgtacgt\n>Beta\nacgttcgt\n");
    }

    #[test]
    fn honours_a_non_default_gap_symbol() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=1 nchar=6;\n format datatype=dna gap=~;\n matrix Alpha AC~~GT ;\nend;\n";
        let opts = Options { remove_gaps: true, ..flat() };
        assert_eq!(convert(src, &opts).unwrap(), ">Alpha\nACGT\n");
    }

    #[test]
    fn rejects_input_with_no_data_block() {
        let err = convert("#NEXUS\nbegin trees;\n tree a = (x,y);\nend;\n", &flat()).unwrap_err();
        assert!(err.contains("no 'begin data;'"), "got: {err}");
    }

    #[test]
    fn reports_a_site_count_mismatch_naming_the_taxon() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=8;\n matrix\n  Alpha ACGTACGT\n  Beta  ACGT\n ;\nend;\n";
        let err = convert(src, &flat()).unwrap_err();
        assert!(err.contains("\"Beta\"") && err.contains("nchar=8"), "got: {err}");
    }

    #[test]
    fn tolerant_converts_despite_a_mismatch() {
        let src = "#NEXUS\nbegin data;\n dimensions ntax=2 nchar=8;\n matrix\n  Alpha ACGTACGT\n  Beta  ACGT\n ;\nend;\n";
        let opts = Options { tolerant: true, ..flat() };
        assert_eq!(convert(src, &opts).unwrap(), ">Alpha\nACGTACGT\n>Beta\nACGT\n");
    }

    #[test]
    fn rejects_an_unclosed_comment() {
        let err = convert("#NEXUS\n[ oops\nbegin data;\n", &flat()).unwrap_err();
        assert!(err.contains("never closed"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(convert("   \n", &flat()).unwrap_err().contains("no input"));
    }

    #[test]
    fn rejects_an_over_wide_wrap() {
        let opts = Options { wrap: MAX_WRAP as usize + 1, ..Options::default() };
        assert!(convert(SEQUENTIAL, &opts).unwrap_err().contains("wrap must be between"));
    }
}
