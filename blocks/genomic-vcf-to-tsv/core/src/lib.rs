//! genomic-vcf-to-tsv core — pure compute, shared by the chat skill block and the web page.
//!
//! Flattens genomic Variant Call Format (VCF 4.x) records into a tidy TSV. The 8 fixed
//! columns (CHROM/POS/ID/REF/ALT/QUAL/FILTER) always lead; the semicolon-separated INFO
//! column is exploded into one column per key (union across records, first-seen order);
//! per-sample FORMAT/genotype data is emitted either LONG (one row per variant × sample,
//! with a SAMPLE column) or WIDE (one row per variant, `<sample>_<FORMATKEY>` columns).
//! No wafer/wasm-bindgen deps — pure string work, deterministic.

/// Options controlling how the VCF is flattened. Mirrors the descriptor params.
pub struct Options<'a> {
    /// "long" (one row per variant × sample) or "wide" (one row per variant).
    pub layout: &'a str,
    /// Explode the INFO column into one column per key.
    pub include_info: bool,
    /// Include per-sample FORMAT/genotype columns.
    pub include_samples: bool,
    /// Comma-separated whitelist of INFO keys to keep; empty = all discovered.
    pub info_fields: &'a str,
    /// Keep only records whose FILTER is PASS or "." (unfiltered); drop the rest.
    pub pass_only: bool,
    /// Prefix every INFO column name with "INFO_" (disambiguates INFO vs FORMAT keys).
    pub prefix_info: bool,
    /// Placeholder text written for an absent/missing value.
    pub missing: &'a str,
    /// Emit a header row of column names.
    pub header: bool,
}

impl<'a> Default for Options<'a> {
    fn default() -> Self {
        Options {
            layout: "long",
            include_info: true,
            include_samples: true,
            info_fields: "",
            pass_only: false,
            prefix_info: false,
            missing: ".",
            header: true,
        }
    }
}

/// Convenience entry point used by the block/CLI and browser wrapper.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    layout: &str,
    include_info: bool,
    include_samples: bool,
    info_fields: &str,
    pass_only: bool,
    prefix_info: bool,
    missing: &str,
    header: bool,
) -> Result<String, String> {
    let opts = Options {
        layout,
        include_info,
        include_samples,
        info_fields,
        pass_only,
        prefix_info,
        missing,
        header,
    };
    vcf_to_tsv(input, &opts)
}

const FIXED: [&str; 7] = ["CHROM", "POS", "ID", "REF", "ALT", "QUAL", "FILTER"];

/// One parsed data record (a VCF variant line).
struct Record {
    fixed: [String; 7],
    /// Parsed INFO key→value pairs (flags map to "true"), in file order.
    info: Vec<(String, String)>,
    /// FORMAT keys for this record (e.g. ["GT","DP"]), empty if no FORMAT column.
    format: Vec<String>,
    /// Raw per-sample value strings (aligned to the header's sample list).
    samples: Vec<String>,
}

/// Flatten VCF text into a TSV string. Returns an error for empty input, a malformed
/// data line (fewer than 8 tab-separated columns), or an unknown `layout`.
pub fn vcf_to_tsv(data: &str, opts: &Options) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste VCF text (meta ## lines, an optional #CHROM header, then variant rows)".into());
    }
    let layout = match opts.layout {
        "" | "long" => "long",
        "wide" => "wide",
        other => return Err(format!("layout must be 'long' or 'wide', got '{other}'")),
    };

    // Header sample names (from the #CHROM line, columns 10+), if present.
    let mut sample_names: Vec<String> = Vec::new();
    let mut saw_header = false;
    let mut records: Vec<Record> = Vec::new();

    for (idx, raw) in data.lines().enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("##") {
            let _ = rest; // meta line — not needed for flattening
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            // The #CHROM column-header line names the samples (fields 9+).
            saw_header = true;
            let cols: Vec<&str> = rest.split('\t').collect();
            if cols.len() > 9 {
                sample_names = cols[9..].iter().map(|s| s.to_string()).collect();
            }
            continue;
        }

        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 8 {
            return Err(format!(
                "line {}: expected at least 8 tab-separated columns (CHROM..INFO), got {} — is this tab-delimited VCF?",
                idx + 1,
                fields.len()
            ));
        }

        // FILTER-based filtering.
        let filter = fields[6];
        if opts.pass_only && filter != "PASS" && filter != "." {
            continue;
        }

        let fixed: [String; 7] = [
            fields[0].to_string(),
            fields[1].to_string(),
            fields[2].to_string(),
            fields[3].to_string(),
            fields[4].to_string(),
            fields[5].to_string(),
            fields[6].to_string(),
        ];

        let info = parse_info(fields[7]);
        let format: Vec<String> = if fields.len() > 8 && fields[8] != "." {
            fields[8].split(':').map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };
        let samples: Vec<String> = if fields.len() > 9 {
            fields[9..].iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };

        records.push(Record {
            fixed,
            info,
            format,
            samples,
        });
    }

    if records.is_empty() {
        if saw_header {
            return Err("no variant records found — the file has a header but no data rows (or pass_only filtered them all out)".into());
        }
        return Err("no variant records found — expected VCF data rows after the header".into());
    }

    // Determine the sample count. Prefer the header; else infer from the widest record.
    let max_record_samples = records.iter().map(|r| r.samples.len()).max().unwrap_or(0);
    if sample_names.is_empty() && max_record_samples > 0 {
        sample_names = (1..=max_record_samples)
            .map(|i| format!("SAMPLE{i}"))
            .collect();
    }
    let has_samples = opts.include_samples && !sample_names.is_empty();

    // Discovered INFO keys (union, first-seen order), optionally whitelisted.
    let info_keys = if opts.include_info {
        discover_info_keys(&records, opts.info_fields)
    } else {
        Vec::new()
    };
    // Discovered FORMAT keys (union, first-seen order).
    let format_keys = if has_samples {
        discover_format_keys(&records)
    } else {
        Vec::new()
    };

    let info_col = |k: &str| -> String {
        if opts.prefix_info {
            format!("INFO_{k}")
        } else {
            k.to_string()
        }
    };

    // Build the header + rows.
    let mut header_cols: Vec<String> = FIXED.iter().map(|s| s.to_string()).collect();
    for k in &info_keys {
        header_cols.push(info_col(k));
    }
    let mut out_rows: Vec<String> = Vec::new();

    if has_samples && layout == "wide" {
        for s in &sample_names {
            for k in &format_keys {
                header_cols.push(format!("{s}_{k}"));
            }
        }
        for r in &records {
            let mut row: Vec<String> = r.fixed.iter().map(|c| clean(c)).collect();
            for k in &info_keys {
                row.push(clean(&lookup_info(r, k, opts.missing)));
            }
            for si in 0..sample_names.len() {
                for k in &format_keys {
                    row.push(clean(&lookup_format(r, si, k, opts.missing)));
                }
            }
            out_rows.push(row.join("\t"));
        }
    } else if has_samples {
        // long: one row per variant × sample.
        header_cols.push("SAMPLE".to_string());
        for k in &format_keys {
            header_cols.push(k.clone());
        }
        for r in &records {
            for (si, sname) in sample_names.iter().enumerate() {
                let mut row: Vec<String> = r.fixed.iter().map(|c| clean(c)).collect();
                for k in &info_keys {
                    row.push(clean(&lookup_info(r, k, opts.missing)));
                }
                row.push(clean(sname));
                for k in &format_keys {
                    row.push(clean(&lookup_format(r, si, k, opts.missing)));
                }
                out_rows.push(row.join("\t"));
            }
        }
    } else {
        // No sample dimension: one row per variant, fixed (+ INFO) columns only.
        for r in &records {
            let mut row: Vec<String> = r.fixed.iter().map(|c| clean(c)).collect();
            for k in &info_keys {
                row.push(clean(&lookup_info(r, k, opts.missing)));
            }
            out_rows.push(row.join("\t"));
        }
    }

    let mut out = String::new();
    if opts.header {
        out.push_str(&header_cols.join("\t"));
        out.push('\n');
    }
    out.push_str(&out_rows.join("\n"));
    Ok(out)
}

/// Parse a VCF INFO field (`key=value;flag;key2=v2`) into ordered pairs. A bare flag
/// (no `=`) maps to the value "true"; an empty/"." INFO field yields no pairs.
fn parse_info(info: &str) -> Vec<(String, String)> {
    if info == "." || info.trim().is_empty() {
        return Vec::new();
    }
    info.split(';')
        .filter(|p| !p.is_empty())
        .map(|part| match part.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (part.to_string(), "true".to_string()),
        })
        .collect()
}

/// Union of INFO keys across records in first-seen order. When `whitelist` is
/// non-empty, keep only those keys (in the whitelist's given order).
fn discover_info_keys(records: &[Record], whitelist: &str) -> Vec<String> {
    let wl: Vec<String> = whitelist
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if !wl.is_empty() {
        return wl;
    }
    let mut seen: Vec<String> = Vec::new();
    for r in records {
        for (k, _) in &r.info {
            if !seen.iter().any(|s| s == k) {
                seen.push(k.clone());
            }
        }
    }
    seen
}

/// Union of FORMAT keys across records in first-seen order.
fn discover_format_keys(records: &[Record]) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in records {
        for k in &r.format {
            if !seen.iter().any(|s| s == k) {
                seen.push(k.clone());
            }
        }
    }
    seen
}

/// Value of INFO key `k` in record `r`, or `missing` if absent.
fn lookup_info(r: &Record, k: &str, missing: &str) -> String {
    r.info
        .iter()
        .find(|(key, _)| key == k)
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| missing.to_string())
}

/// Value of FORMAT key `k` for sample index `si` in record `r`, or `missing`.
fn lookup_format(r: &Record, si: usize, k: &str, missing: &str) -> String {
    let pos = match r.format.iter().position(|f| f == k) {
        Some(p) => p,
        None => return missing.to_string(),
    };
    let raw = match r.samples.get(si) {
        Some(s) if s != "." && !s.is_empty() => s,
        _ => return missing.to_string(),
    };
    raw.split(':')
        .nth(pos)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| missing.to_string())
}

/// Make a cell TSV-safe: a tab or newline inside a value would corrupt the columns,
/// so replace either with a single space (VCF fields should not contain them).
fn clean(v: &str) -> String {
    if v.contains('\t') || v.contains('\n') || v.contains('\r') {
        v.replace(['\t', '\n', '\r'], " ")
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VCF: &str = "\
##fileformat=VCFv4.2
##INFO=<ID=DP,Number=1,Type=Integer,Description=\"Depth\">
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\tFORMAT\tNA001\tNA002
chr1\t100\trs1\tA\tG\t50\tPASS\tDP=30;AF=0.5\tGT:DP\t0/1:20\t1/1:10
chr2\t200\t.\tC\tT\t99\tq10\tDP=12\tGT:DP\t0/0:12\t./.";

    #[test]
    fn long_layout_flattens_info_and_samples() {
        let out = vcf_to_tsv(SAMPLE_VCF, &Options::default()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tDP\tAF\tSAMPLE\tGT\tDP"
        );
        // 2 variants × 2 samples = 4 data rows.
        assert_eq!(lines.len(), 5);
        // First variant, sample NA001: DP(info)=30, AF=0.5, GT=0/1, DP(format)=20.
        assert_eq!(
            lines[1],
            "chr1\t100\trs1\tA\tG\t50\tPASS\t30\t0.5\tNA001\t0/1\t20"
        );
        // Second variant lacks AF → missing "."; NA002 is the no-call GT "./."
        // (preserved literally) and has no DP subfield → DP missing ".".
        assert_eq!(
            lines[4],
            "chr2\t200\t.\tC\tT\t99\tq10\t12\t.\tNA002\t./.\t."
        );
    }

    #[test]
    fn wide_layout_one_row_per_variant() {
        let opts = Options {
            layout: "wide",
            ..Options::default()
        };
        let out = vcf_to_tsv(SAMPLE_VCF, &opts).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tDP\tAF\tNA001_GT\tNA001_DP\tNA002_GT\tNA002_DP"
        );
        assert_eq!(lines.len(), 3); // header + 2 variants
        assert_eq!(
            lines[1],
            "chr1\t100\trs1\tA\tG\t50\tPASS\t30\t0.5\t0/1\t20\t1/1\t10"
        );
    }

    #[test]
    fn pass_only_drops_non_pass() {
        let opts = Options {
            pass_only: true,
            include_samples: false,
            include_info: false,
            ..Options::default()
        };
        let out = vcf_to_tsv(SAMPLE_VCF, &opts).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // Only chr1 is PASS; chr2 is q10.
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER");
        assert_eq!(lines[1], "chr1\t100\trs1\tA\tG\t50\tPASS");
    }

    #[test]
    fn info_whitelist_and_prefix() {
        let opts = Options {
            include_samples: false,
            info_fields: "AF",
            prefix_info: true,
            ..Options::default()
        };
        let out = vcf_to_tsv(SAMPLE_VCF, &opts).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO_AF");
        assert_eq!(lines[1], "chr1\t100\trs1\tA\tG\t50\tPASS\t0.5");
        assert_eq!(lines[2], "chr2\t200\t.\tC\tT\t99\tq10\t."); // no AF → missing
    }

    #[test]
    fn flag_info_and_custom_missing() {
        let vcf = "\
#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO
chr1\t5\t.\tA\tT\t.\t.\tDB;DP=9
chr1\t6\t.\tG\tC\t.\t.\tDP=4";
        let opts = Options {
            missing: "NA",
            ..Options::default()
        };
        let out = vcf_to_tsv(vcf, &opts).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tDB\tDP");
        assert_eq!(lines[1], "chr1\t5\t.\tA\tT\t.\t.\ttrue\t9"); // flag present → true
        assert_eq!(lines[2], "chr1\t6\t.\tG\tC\t.\t.\tNA\t4"); // flag absent → NA
    }

    #[test]
    fn no_header_line_infers_sample_names() {
        let vcf = "chr1\t1\t.\tA\tT\t.\t.\t.\tGT\t0/1";
        let out = vcf_to_tsv(vcf, &Options::default()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tSAMPLE\tGT"
        );
        assert_eq!(lines[1], "chr1\t1\t.\tA\tT\t.\t.\tSAMPLE1\t0/1");
    }

    #[test]
    fn header_can_be_omitted() {
        let opts = Options {
            header: false,
            include_info: false,
            include_samples: false,
            ..Options::default()
        };
        let out = vcf_to_tsv(SAMPLE_VCF, &opts).unwrap();
        assert!(out.starts_with("chr1\t100"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(vcf_to_tsv("   \n  ", &Options::default()).is_err());
    }

    #[test]
    fn malformed_line_errors() {
        let vcf = "#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO\nchr1\t100\trs1";
        let err = vcf_to_tsv(vcf, &Options::default()).unwrap_err();
        assert!(err.contains("at least 8"));
    }

    #[test]
    fn header_only_no_records_errors() {
        let vcf = "##fileformat=VCFv4.2\n#CHROM\tPOS\tID\tREF\tALT\tQUAL\tFILTER\tINFO";
        assert!(vcf_to_tsv(vcf, &Options::default()).is_err());
    }

    #[test]
    fn bad_layout_errors() {
        let opts = Options {
            layout: "tall",
            ..Options::default()
        };
        assert!(vcf_to_tsv(SAMPLE_VCF, &opts).is_err());
    }
}
