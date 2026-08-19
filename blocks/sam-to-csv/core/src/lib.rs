//! sam-to-csv core — pure compute, shared by the chat skill block and the web page.
//!
//! Parses SAM (Sequence Alignment/Map) text into a delimited table with named
//! columns. `@` header lines are skipped; every alignment line contributes one
//! row built from the 11 mandatory fields (QNAME, FLAG, RNAME, POS, MAPQ,
//! CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL), optionally followed by decoded FLAG
//! bits, columns computed from POS/CIGAR/SEQ, and the optional TAG:TYPE:VALUE
//! fields either joined into one cell or expanded one column per tag.

/// Hard cap on alignment records per run (keeps the browser responsive).
pub const MAX_RECORDS: usize = 20_000;

/// The 12 SAM FLAG bits, low bit first, with the short names used for columns.
pub const FLAG_BITS: [(u16, &str); 12] = [
    (0x1, "PAIRED"),
    (0x2, "PROPER_PAIR"),
    (0x4, "UNMAPPED"),
    (0x8, "MATE_UNMAPPED"),
    (0x10, "REVERSE"),
    (0x20, "MATE_REVERSE"),
    (0x40, "READ1"),
    (0x80, "READ2"),
    (0x100, "SECONDARY"),
    (0x200, "QCFAIL"),
    (0x400, "DUPLICATE"),
    (0x800, "SUPPLEMENTARY"),
];

const MANDATORY: [&str; 11] = [
    "QNAME", "FLAG", "RNAME", "POS", "MAPQ", "CIGAR", "RNEXT", "PNEXT", "TLEN", "SEQ", "QUAL",
];

/// Names of the bits set in `flag`, low bit first.
pub fn flag_names(flag: u16) -> Vec<&'static str> {
    FLAG_BITS
        .iter()
        .filter(|(bit, _)| flag & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

fn parse_delimiter(spec: &str) -> Result<char, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "" | "comma" | "," => Ok(','),
        "tab" | "\t" | "\\t" => Ok('\t'),
        "semicolon" | ";" => Ok(';'),
        "pipe" | "|" => Ok('|'),
        other => Err(format!(
            "unknown delimiter '{other}' (expected comma, tab, semicolon, or pipe)"
        )),
    }
}

fn quote(value: &str, delim: char) -> String {
    let needs = value.contains(delim)
        || value.contains('"')
        || value.contains('\n')
        || value.contains('\r')
        || value.starts_with(' ')
        || value.ends_with(' ');
    if needs {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Reference bases consumed by a CIGAR string (M, D, N, =, X) and query bases
/// consumed (M, I, S, =, X). Returns `(reference_span, query_span)`.
fn cigar_spans(cigar: &str, line_no: usize) -> Result<(i64, i64), String> {
    if cigar == "*" {
        return Ok((0, 0));
    }
    let mut reference = 0i64;
    let mut query = 0i64;
    let mut digits = String::new();
    for ch in cigar.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            continue;
        }
        if digits.is_empty() {
            return Err(format!(
                "line {line_no}: CIGAR '{cigar}' has operation '{ch}' with no length before it (expected forms like 36M or 5S30M2I)"
            ));
        }
        let len: i64 = digits.parse().map_err(|_| {
            format!("line {line_no}: CIGAR '{cigar}' has a length '{digits}' that is too large")
        })?;
        digits.clear();
        match ch {
            'M' | '=' | 'X' => {
                reference += len;
                query += len;
            }
            'D' | 'N' => reference += len,
            'I' | 'S' => query += len,
            'H' | 'P' => {}
            other => {
                return Err(format!(
                    "line {line_no}: CIGAR '{cigar}' has an unknown operation '{other}' (expected M, I, D, N, S, H, P, = or X)"
                ))
            }
        }
    }
    if !digits.is_empty() {
        return Err(format!(
            "line {line_no}: CIGAR '{cigar}' ends with a length '{digits}' that has no operation letter"
        ));
    }
    Ok((reference, query))
}

fn parse_int(value: &str, field: &str, line_no: usize) -> Result<i64, String> {
    value.trim().parse::<i64>().map_err(|_| {
        format!("line {line_no}: {field} '{value}' is not an integer (SAM field {field} must be numeric)")
    })
}

struct Record {
    cells: Vec<String>,
    flag: u16,
    mapq: i64,
    pos: i64,
    cigar: String,
    seq: String,
    ref_span: i64,
    query_span: i64,
    tags: Vec<(String, String)>,
}

fn split_fields(line: &str) -> Vec<&str> {
    let tabbed: Vec<&str> = line.split('\t').collect();
    if tabbed.len() >= 11 {
        return tabbed;
    }
    // Tolerate SAM pasted from a web page or PDF, where the tabs became runs of
    // spaces. Tag values containing spaces cannot survive this fallback.
    let spaced: Vec<&str> = line.split_whitespace().collect();
    if spaced.len() >= 11 {
        return spaced;
    }
    tabbed
}

fn parse_record(line: &str, line_no: usize) -> Result<Record, String> {
    let fields = split_fields(line);
    if fields.len() < 11 {
        return Err(format!(
            "line {line_no}: found {} field(s) but a SAM alignment record needs 11 (QNAME, FLAG, RNAME, POS, MAPQ, CIGAR, RNEXT, PNEXT, TLEN, SEQ, QUAL) — check that the columns are separated by tabs",
            fields.len()
        ));
    }
    let flag_raw = fields[1].trim();
    let flag: u16 = flag_raw.parse().map_err(|_| {
        format!("line {line_no}: FLAG '{flag_raw}' is not a whole number in 0-65535 (it is a bitwise sum such as 99)")
    })?;
    let pos = parse_int(fields[3], "POS", line_no)?;
    let mapq = parse_int(fields[4], "MAPQ", line_no)?;
    if !(0..=255).contains(&mapq) {
        return Err(format!(
            "line {line_no}: MAPQ '{mapq}' is out of range (expected 0-255, where 255 means unavailable)"
        ));
    }
    parse_int(fields[7], "PNEXT", line_no)?;
    parse_int(fields[8], "TLEN", line_no)?;

    let mut tags: Vec<(String, String)> = Vec::new();
    for token in &fields[11..] {
        if token.is_empty() {
            continue;
        }
        let parts: Vec<&str> = token.splitn(3, ':').collect();
        if parts.len() != 3 || parts[0].len() != 2 || parts[1].len() != 1 {
            return Err(format!(
                "line {line_no}: optional field '{token}' is not in TAG:TYPE:VALUE form (for example NM:i:0 or MD:Z:36)"
            ));
        }
        let name = parts[0].to_string();
        if let Some(slot) = tags.iter_mut().find(|(n, _)| *n == name) {
            slot.1 = parts[2].to_string();
        } else {
            tags.push((name, parts[2].to_string()));
        }
    }

    let cigar = fields[5].trim().to_string();
    let (ref_span, query_span) = cigar_spans(&cigar, line_no)?;

    Ok(Record {
        cells: fields[..11].iter().map(|f| f.trim().to_string()).collect(),
        flag,
        mapq,
        pos,
        cigar,
        seq: fields[9].trim().to_string(),
        ref_span,
        query_span,
        tags,
    })
}

fn mode(value: &str, allowed: &[&str], name: &str, default: &str) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    let v = if v.is_empty() { default.to_string() } else { v };
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "unknown {name} '{v}' (expected one of: {})",
            allowed.join(", ")
        ))
    }
}

/// Convert SAM text into a delimited table.
///
/// * `delimiter` — `comma` | `tab` | `semicolon` | `pipe`
/// * `flags` — `none` | `summary` | `bits` | `both` (decoded FLAG columns)
/// * `tags` — `none` | `joined` | `expand` (optional TAG:TYPE:VALUE fields)
/// * `tag_fields` — comma-separated whitelist of tag names, in output order
/// * `include_seq` — keep the SEQ and QUAL columns
/// * `computed` — add END, REF_SPAN, READ_LEN and STRAND
/// * `mapped_only` / `primary_only` / `min_mapq` — record filters
/// * `missing` — placeholder for values that do not apply
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    delimiter: &str,
    header: bool,
    flags: &str,
    tags: &str,
    tag_fields: &str,
    include_seq: bool,
    computed: bool,
    mapped_only: bool,
    primary_only: bool,
    min_mapq: u32,
    missing: &str,
) -> Result<String, String> {
    let delim = parse_delimiter(delimiter)?;
    let flags = mode(flags, &["none", "summary", "bits", "both"], "flags mode", "summary")?;
    let tags_mode = mode(tags, &["none", "joined", "expand"], "tags mode", "expand")?;
    if min_mapq > 255 {
        return Err(format!(
            "min_mapq {min_mapq} is out of range (expected 0-255)"
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty — paste SAM alignment records (tab-separated, @ header lines optional)".into());
    }

    let whitelist: Vec<String> = tag_fields
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let mut records: Vec<Record> = Vec::new();
    let mut header_lines = 0usize;
    for (idx, raw) in input.lines().enumerate() {
        let line = raw.trim_end_matches(['\r', ' ']);
        if line.trim().is_empty() {
            continue;
        }
        if line.starts_with('@') {
            header_lines += 1;
            continue;
        }
        if records.len() == MAX_RECORDS {
            return Err(format!(
                "too many alignment records (cap is {MAX_RECORDS}) — convert the file in smaller batches"
            ));
        }
        records.push(parse_record(line, idx + 1)?);
    }
    if records.is_empty() {
        return Err(format!(
            "no alignment records found (skipped {header_lines} @ header line(s)) — paste the alignment lines that follow the header"
        ));
    }

    // Filters.
    records.retain(|r| {
        if mapped_only && r.flag & 0x4 != 0 {
            return false;
        }
        if primary_only && (r.flag & 0x100 != 0 || r.flag & 0x800 != 0) {
            return false;
        }
        r.mapq >= min_mapq as i64
    });

    // Tag column order: whitelist order when given, else first-seen order.
    let mut tag_cols: Vec<String> = Vec::new();
    if tags_mode == "expand" {
        if whitelist.is_empty() {
            for r in &records {
                for (name, _) in &r.tags {
                    if !tag_cols.contains(name) {
                        tag_cols.push(name.clone());
                    }
                }
            }
        } else {
            tag_cols = whitelist.clone();
        }
    }

    let mut columns: Vec<String> = MANDATORY
        .iter()
        .filter(|c| include_seq || (**c != "SEQ" && **c != "QUAL"))
        .map(|c| c.to_string())
        .collect();
    if flags == "summary" || flags == "both" {
        columns.push("FLAG_SUMMARY".into());
    }
    if flags == "bits" || flags == "both" {
        for (_, name) in FLAG_BITS {
            columns.push(format!("FLAG_{name}"));
        }
    }
    if computed {
        columns.extend(["END", "REF_SPAN", "READ_LEN", "STRAND"].map(String::from));
    }
    match tags_mode.as_str() {
        "joined" => columns.push("TAGS".into()),
        "expand" => columns.extend(tag_cols.iter().cloned()),
        _ => {}
    }

    let mut out: Vec<String> = Vec::with_capacity(records.len() + 1);
    if header {
        out.push(
            columns
                .iter()
                .map(|c| quote(c, delim))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
    }

    for r in &records {
        let mut row: Vec<String> = Vec::with_capacity(columns.len());
        for (i, name) in MANDATORY.iter().enumerate() {
            if !include_seq && (*name == "SEQ" || *name == "QUAL") {
                continue;
            }
            row.push(r.cells[i].clone());
        }
        if flags == "summary" || flags == "both" {
            let names = flag_names(r.flag);
            row.push(if names.is_empty() {
                missing.to_string()
            } else {
                names.join(",")
            });
        }
        if flags == "bits" || flags == "both" {
            for (bit, _) in FLAG_BITS {
                row.push(if r.flag & bit != 0 { "true" } else { "false" }.to_string());
            }
        }
        if computed {
            let (ref_span, query_span) = (r.ref_span, r.query_span);
            let unmapped = r.flag & 0x4 != 0;
            if r.cigar == "*" || ref_span == 0 || r.pos == 0 {
                row.push(missing.to_string());
                row.push(missing.to_string());
            } else {
                row.push((r.pos + ref_span - 1).to_string());
                row.push(ref_span.to_string());
            }
            let read_len = if r.seq != "*" {
                r.seq.chars().count() as i64
            } else {
                query_span
            };
            row.push(if read_len == 0 {
                missing.to_string()
            } else {
                read_len.to_string()
            });
            row.push(if unmapped {
                missing.to_string()
            } else if r.flag & 0x10 != 0 {
                "-".into()
            } else {
                "+".into()
            });
        }
        match tags_mode.as_str() {
            "joined" => {
                let kept: Vec<String> = r
                    .tags
                    .iter()
                    .filter(|(n, _)| whitelist.is_empty() || whitelist.contains(n))
                    .map(|(n, v)| format!("{n}:{v}"))
                    .collect();
                row.push(if kept.is_empty() {
                    missing.to_string()
                } else {
                    kept.join(" ")
                });
            }
            "expand" => {
                for col in &tag_cols {
                    let value = r
                        .tags
                        .iter()
                        .find(|(n, _)| n == col)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_else(|| missing.to_string());
                    row.push(value);
                }
            }
            _ => {}
        }
        out.push(
            row.iter()
                .map(|c| quote(c, delim))
                .collect::<Vec<_>>()
                .join(&delim.to_string()),
        );
    }

    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAM: &str = "@HD\tVN:1.6\tSO:coordinate\n@SQ\tSN:chr1\tLN:248956422\nr001\t99\tchr1\t7\t60\t8M2I4M1D3M\t=\t37\t39\tTTAGATAAAGGATACTG\t*\tNM:i:1\tAS:i:30\nr002\t0\tchr1\t9\t30\t3S6M1P1I4M\t*\t0\t0\tAAAAGATAAGGATA\t*\tNM:i:0";

    fn defaults(input: &str) -> Result<String, String> {
        run(input, "comma", true, "summary", "expand", "", true, false, false, false, 0, ".")
    }

    #[test]
    fn columnizes_mandatory_fields_with_header() {
        let out = defaults(SAM).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "QNAME,FLAG,RNAME,POS,MAPQ,CIGAR,RNEXT,PNEXT,TLEN,SEQ,QUAL,FLAG_SUMMARY,NM,AS"
        );
        assert_eq!(
            lines[1],
            "r001,99,chr1,7,60,8M2I4M1D3M,=,37,39,TTAGATAAAGGATACTG,*,\"PAIRED,PROPER_PAIR,MATE_REVERSE,READ1\",1,30"
        );
        // r002 has no AS tag → the missing placeholder fills the column.
        assert_eq!(
            lines[2],
            "r002,0,chr1,9,30,3S6M1P1I4M,*,0,0,AAAAGATAAGGATA,*,.,0,."
        );
        assert_eq!(lines.len(), 3, "@ header lines are skipped");
    }

    #[test]
    fn space_separated_paste_still_parses() {
        let out = defaults("r1 4 * 0 0 * * 0 0 ACGT IIII").unwrap();
        assert_eq!(out.lines().nth(1).unwrap(), "r1,4,*,0,0,*,*,0,0,ACGT,IIII,UNMAPPED");
    }

    #[test]
    fn flag_bits_columns_decode_every_bit() {
        let out = run(SAM, "comma", true, "bits", "none", "", false, false, false, false, 0, ".")
            .unwrap();
        let head: Vec<&str> = out.lines().next().unwrap().split(',').collect();
        assert_eq!(head[9], "FLAG_PAIRED");
        assert_eq!(head[20], "FLAG_SUPPLEMENTARY");
        let row: Vec<&str> = out.lines().nth(1).unwrap().split(',').collect();
        assert_eq!(row[9], "true", "0x1 PAIRED set in 99");
        assert_eq!(row[11], "false", "0x4 UNMAPPED not set in 99");
        assert_eq!(row[14], "true", "0x20 MATE_REVERSE set in 99");
        assert!(!out.contains("SEQ"), "include_seq=false drops SEQ/QUAL");
    }

    #[test]
    fn computed_columns_use_pos_cigar_and_seq() {
        let out = run(SAM, "tab", true, "none", "none", "", false, true, false, false, 0, ".")
            .unwrap();
        let head: Vec<&str> = out.lines().next().unwrap().split('\t').collect();
        assert_eq!(&head[9..], ["END", "REF_SPAN", "READ_LEN", "STRAND"]);
        let row: Vec<&str> = out.lines().nth(1).unwrap().split('\t').collect();
        // 8M2I4M1D3M consumes 8+4+1+3 = 16 reference bases from POS 7 → END 22.
        assert_eq!(&row[9..], ["22", "16", "17", "+"]);
    }

    #[test]
    fn joined_tags_and_whitelist() {
        let out = run(SAM, "comma", false, "none", "joined", "NM", true, false, false, false, 0, ".")
            .unwrap();
        let row: Vec<&str> = out.lines().next().unwrap().split(',').collect();
        assert_eq!(row[11], "NM:1", "AS filtered out by the whitelist");
    }

    #[test]
    fn filters_drop_unmapped_secondary_and_low_mapq() {
        let sam = "r1\t0\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*\nr2\t4\t*\t0\t0\t*\t*\t0\t0\tACGT\t*\nr3\t256\tchr1\t20\t60\t4M\t*\t0\t0\tACGT\t*\nr4\t0\tchr1\t30\t5\t4M\t*\t0\t0\tACGT\t*";
        let out = run(sam, "comma", false, "none", "none", "", false, false, true, true, 10, ".")
            .unwrap();
        let names: Vec<&str> = out.lines().map(|l| l.split(',').next().unwrap()).collect();
        assert_eq!(names, ["r1"], "unmapped, secondary and MAPQ<10 dropped");
    }

    #[test]
    fn quotes_cells_containing_the_delimiter() {
        let sam = "r1\t0\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*\tCO:Z:hello, world";
        let out = run(sam, "comma", false, "none", "expand", "", false, false, false, false, 0, ".")
            .unwrap();
        assert!(out.ends_with("\"hello, world\""), "got {out}");
    }

    #[test]
    fn rejects_records_with_too_few_fields() {
        let err = defaults("r1\t0\tchr1").unwrap_err();
        assert!(err.contains("line 1"), "got {err}");
        assert!(err.contains("needs 11"), "got {err}");
    }

    #[test]
    fn rejects_non_numeric_flag() {
        let err = defaults("r1\tbad\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*").unwrap_err();
        assert!(err.contains("FLAG 'bad'"), "got {err}");
    }

    #[test]
    fn rejects_bad_cigar_operation() {
        let err = run(
            "r1\t0\tchr1\t10\t60\t4Z\t*\t0\t0\tACGT\t*",
            "comma", false, "none", "none", "", false, true, false, false, 0, ".",
        )
        .unwrap_err();
        assert!(err.contains("unknown operation 'Z'"), "got {err}");
    }

    #[test]
    fn rejects_malformed_optional_field() {
        let err = defaults("r1\t0\tchr1\t10\t60\t4M\t*\t0\t0\tACGT\t*\tNM=1").unwrap_err();
        assert!(err.contains("TAG:TYPE:VALUE"), "got {err}");
    }

    #[test]
    fn rejects_header_only_and_empty_input() {
        assert!(defaults("@HD\tVN:1.6").unwrap_err().contains("no alignment records"));
        assert!(defaults("   ").unwrap_err().contains("input is empty"));
    }

    #[test]
    fn rejects_unknown_delimiter_and_mode() {
        assert!(run(SAM, "colon", true, "summary", "expand", "", true, false, false, false, 0, ".")
            .unwrap_err()
            .contains("unknown delimiter"));
        assert!(run(SAM, "comma", true, "verbose", "expand", "", true, false, false, false, 0, ".")
            .unwrap_err()
            .contains("unknown flags mode"));
    }
}
