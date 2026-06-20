//! gizza-ai/file-splitter core — pure-Rust splitting of a text/CSV file into
//! pieces along line boundaries. No deps. Three modes:
//!   - `Parts`  — split into `count` roughly-equal parts (by line count).
//!   - `Lines`  — each piece has up to `count` lines.
//!   - `Bytes`  — each piece is as many whole lines as fit in `count` bytes
//!                (a single over-long line still forms its own piece).
//! With `csv_header`, the first line is treated as a header and repeated at the
//! top of every piece. Splitting always happens on line boundaries so no line is
//! ever cut in half; exact bytes (including line endings) are preserved.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Parts,
    Lines,
    Bytes,
}

/// Hard cap on the number of output pieces (guards against silly `count` values).
const MAX_PIECES: usize = 10_000;

pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "parts" => Ok(Mode::Parts),
        "lines" => Ok(Mode::Lines),
        "bytes" => Ok(Mode::Bytes),
        other => Err(format!("mode {other:?} not supported (parts|lines|bytes)")),
    }
}

/// Split `text` per `mode`/`count`. Returns the pieces' contents in order.
pub fn split(text: &str, mode: Mode, count: usize, csv_header: bool) -> Result<Vec<String>, String> {
    if text.is_empty() {
        return Err("input is empty".into());
    }
    if count == 0 {
        return Err("count must be at least 1".into());
    }

    // Line units that keep their trailing '\n' so concatenation is byte-exact.
    let units: Vec<&str> = text.split_inclusive('\n').collect();

    let (header, body): (Option<&str>, &[&str]) = if csv_header {
        match units.split_first() {
            Some((h, rest)) => (Some(*h), rest),
            None => (None, &[]),
        }
    } else {
        (None, &units[..])
    };

    if body.is_empty() {
        return Err("nothing to split (no data rows after the header)".into());
    }

    // Group body line-units into pieces.
    let groups: Vec<&[&str]> = match mode {
        Mode::Parts => {
            if count > body.len() {
                return Err(format!(
                    "cannot split {} line(s) into {count} parts (more parts than lines)",
                    body.len()
                ));
            }
            // Even distribution: the first `rem` parts get one extra line.
            let base = body.len() / count;
            let rem = body.len() % count;
            let mut out = Vec::with_capacity(count);
            let mut i = 0;
            for p in 0..count {
                let take = base + if p < rem { 1 } else { 0 };
                out.push(&body[i..i + take]);
                i += take;
            }
            out
        }
        Mode::Lines => body.chunks(count).collect(),
        Mode::Bytes => {
            let mut out: Vec<&[&str]> = Vec::new();
            let mut start = 0usize;
            let mut cur_bytes = 0usize;
            for (i, u) in body.iter().enumerate() {
                let ulen = u.len();
                // Start a new piece if the current one is non-empty and adding
                // this line would exceed the byte budget.
                if i > start && cur_bytes + ulen > count {
                    out.push(&body[start..i]);
                    start = i;
                    cur_bytes = 0;
                }
                cur_bytes += ulen;
            }
            if start < body.len() {
                out.push(&body[start..]);
            }
            out
        }
    };

    if groups.len() > MAX_PIECES {
        return Err(format!(
            "that would produce {} pieces (max {MAX_PIECES}); use a larger chunk size",
            groups.len()
        ));
    }

    let pieces = groups
        .into_iter()
        .map(|g| {
            let mut s = String::new();
            if let Some(h) = header {
                s.push_str(h);
            }
            for u in g {
                s.push_str(u);
            }
            s
        })
        .collect();
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_even_distribution() {
        let t = "a\nb\nc\nd\ne\n";
        let p = split(t, Mode::Parts, 2, false).unwrap();
        assert_eq!(p.len(), 2);
        // 5 lines into 2 parts → 3 + 2
        assert_eq!(p[0], "a\nb\nc\n");
        assert_eq!(p[1], "d\ne\n");
    }

    #[test]
    fn parts_more_than_lines_errors() {
        assert!(split("a\nb\n", Mode::Parts, 5, false).is_err());
    }

    #[test]
    fn lines_mode_chunks_by_count() {
        let t = "1\n2\n3\n4\n5";
        let p = split(t, Mode::Lines, 2, false).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p[0], "1\n2\n");
        assert_eq!(p[1], "3\n4\n");
        assert_eq!(p[2], "5"); // last line has no trailing newline — preserved
    }

    #[test]
    fn bytes_mode_splits_on_line_boundary() {
        // Each line "aa\n" is 3 bytes; budget 7 → 2 lines (6B) per piece.
        let t = "aa\naa\naa\naa\n";
        let p = split(t, Mode::Bytes, 7, false).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], "aa\naa\n");
        assert_eq!(p[1], "aa\naa\n");
    }

    #[test]
    fn bytes_mode_overlong_line_is_own_piece() {
        let t = "short\nthisisaverylongline\nshort\n";
        let p = split(t, Mode::Bytes, 8, false).unwrap();
        // "short\n"(6) alone; long line(19) alone; "short\n"(6) alone
        assert_eq!(p.len(), 3);
        assert_eq!(p[1], "thisisaverylongline\n");
    }

    #[test]
    fn csv_header_repeated_in_each_piece() {
        let t = "id,name\n1,a\n2,b\n3,c\n4,d\n";
        let p = split(t, Mode::Parts, 2, true).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0], "id,name\n1,a\n2,b\n");
        assert_eq!(p[1], "id,name\n3,c\n4,d\n");
    }

    #[test]
    fn errors() {
        assert!(split("", Mode::Parts, 2, false).is_err());
        assert!(split("a\n", Mode::Lines, 0, false).is_err());
        // header-only CSV has no body rows
        assert!(split("id,name\n", Mode::Parts, 1, true).is_err());
    }

    #[test]
    fn round_trips_concatenation() {
        let t = "alpha\nbeta\ngamma\ndelta\n";
        let p = split(t, Mode::Lines, 2, false).unwrap();
        assert_eq!(p.concat(), t, "pieces must reassemble to the original");
    }

    #[test]
    fn parse_mode_values() {
        assert_eq!(parse_mode("parts").unwrap(), Mode::Parts);
        assert_eq!(parse_mode("").unwrap(), Mode::Parts);
        assert_eq!(parse_mode("LINES").unwrap(), Mode::Lines);
        assert_eq!(parse_mode("bytes").unwrap(), Mode::Bytes);
        assert!(parse_mode("words").is_err());
    }
}
