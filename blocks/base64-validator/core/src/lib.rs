//! gizza-ai/base64-validator core — decide whether a string is well-formed
//! Base64 (RFC 4648 §4) or Base64url (§5) and, when it isn't, say exactly what
//! is wrong: which character, at which position, and how the padding or length
//! is off.
//!
//! Pure compute, no dependencies (the Base64 decode + JSON rendering are both
//! hand-rolled). Invalid input is a RESULT, not an error — the tool always
//! returns a report. `Err` is reserved for an empty input, an over-size input,
//! or an unknown option value.
//!
//! * `variant`: `auto` (default, detect and report which alphabet) | `standard`
//!   (`+` and `/`) | `url-safe` (`-` and `_`).
//! * `padding`: `optional` (default) | `required` (strict RFC 4648) |
//!   `forbidden` (unpadded Base64url, e.g. a JWT segment).
//! * `ignore_whitespace`: strip spaces/tabs/newlines inside the data (default
//!   true); when false each one is reported as an invalid character.
//! * `max_line_length`: 0 (default) = no check, 64 = PEM, 76 = MIME.
//! * `output`: `text` (default) or `json`.

/// Longest input accepted, in bytes (~3 MB of decoded payload).
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Most individual problems listed before the rest are summarised as a count.
const MAX_PROBLEMS: usize = 20;
/// Longest decoded-text preview, in characters.
const TEXT_PREVIEW_CHARS: usize = 64;
/// Longest decoded-binary preview, in bytes.
const HEX_PREVIEW_BYTES: usize = 24;

/// Which alphabet the caller wants enforced.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Auto,
    Standard,
    UrlSafe,
}

/// What the caller expects of the trailing `=` characters.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Padding {
    Optional,
    Required,
    Forbidden,
}

/// One thing that is wrong with the input.
struct Problem {
    message: String,
    /// 1-based character position in the ORIGINAL input, when the problem is
    /// tied to a single character.
    position: Option<usize>,
    line: Option<usize>,
    column: Option<usize>,
}

impl Problem {
    fn at(message: impl Into<String>, position: usize, line: usize, column: usize) -> Self {
        Problem {
            message: message.into(),
            position: Some(position),
            line: Some(line),
            column: Some(column),
        }
    }
    fn general(message: impl Into<String>) -> Self {
        Problem {
            message: message.into(),
            position: None,
            line: None,
            column: None,
        }
    }
}

/// Validate `input`, rendering the report as `output` (`text` | `json`).
pub fn validate(
    input: &str,
    variant: &str,
    padding: &str,
    ignore_whitespace: bool,
    max_line_length: i64,
    output: &str,
) -> Result<String, String> {
    let variant = parse_variant(variant)?;
    let pad_policy = parse_padding(padding)?;
    let as_json = parse_output(output)?;
    if !(0..=998).contains(&max_line_length) {
        return Err(format!(
            "invalid max_line_length {max_line_length}: expected 0 (no check) to 998, e.g. 76 for MIME or 64 for PEM"
        ));
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large: {} bytes, limit is {} bytes ({} MiB)",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if input.trim().is_empty() {
        return Err(
            "input is empty: paste a Base64 string to validate, e.g. 'SGVsbG8sIHdvcmxkIQ=='".into(),
        );
    }

    let mut notes: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut problems: Vec<Problem> = Vec::new();

    // ---- strip what is around the payload, remembering how far in it starts.
    let lead_ws = input.len()
        - input
            .trim_start_matches(|c: char| c.is_ascii_whitespace())
            .len();
    let trimmed = input.trim_matches(|c: char| c.is_ascii_whitespace());
    let mut start = lead_ws;
    let mut payload = trimmed;
    if !ignore_whitespace && input.len() != trimmed.len() {
        notes.push("Leading and trailing whitespace was trimmed before the strict check.".into());
    }

    if payload.len() >= 2 {
        let b = payload.as_bytes();
        let quote = b[0];
        if (quote == b'"' || quote == b'\'') && b[b.len() - 1] == quote {
            payload = &payload[1..payload.len() - 1];
            start += 1;
            notes.push(format!(
                "Removed the wrapping {} quotes.",
                if quote == b'"' { "double" } else { "single" }
            ));
        }
    }

    if payload.len() >= 5 && payload.as_bytes()[..5].eq_ignore_ascii_case(b"data:") {
        if let Some(at) = find_ci(payload, ";base64,", 1024) {
            let cut = at + ";base64,".len();
            notes.push(format!(
                "Detected a data: URI — validated the Base64 payload after '{}'.",
                &payload[at..cut]
            ));
            start += cut;
            payload = &payload[cut..];
        }
    }

    if payload.trim().is_empty() {
        return Err("no Base64 payload found: the input is only a prefix or quotes".into());
    }

    // ---- scan the payload, one pass, tracking position/line/column.
    let (line, mut column) = line_col_at(input, start);
    let mut pos = input[..start].chars().count(); // chars consumed before the payload

    let mut data = String::with_capacity(payload.len());
    let mut n_std = 0usize; // '+' or '/'
    let mut n_url = 0usize; // '-' or '_'
    let mut n_pad = 0usize; // '='
    let mut whitespace_removed = 0usize;
    let mut first_std: Option<(char, usize, usize, usize)> = None;
    let mut first_url: Option<(char, usize, usize, usize)> = None;
    let mut first_pad: Option<usize> = None;
    let mut data_after_pad: Option<(usize, usize)> = None; // (pad position, data position)
    let mut bad_chars = 0usize;

    for ch in payload.chars() {
        pos += 1;
        column += 1;
        let here = (pos, line, column);

        match classify(ch) {
            class @ (Class::Shared | Class::Standard | Class::UrlSafe) => {
                if let Some(p) = first_pad {
                    if data_after_pad.is_none() {
                        data_after_pad = Some((p, pos));
                    }
                }
                match class {
                    Class::Standard => {
                        n_std += 1;
                        let _ = first_std.get_or_insert((ch, here.0, here.1, here.2));
                        data.push(ch);
                    }
                    Class::UrlSafe => {
                        n_url += 1;
                        let _ = first_url.get_or_insert((ch, here.0, here.1, here.2));
                        data.push(if ch == '-' { '+' } else { '/' });
                    }
                    _ => data.push(ch),
                }
            }
            Class::Pad => {
                n_pad += 1;
                first_pad.get_or_insert(pos);
            }
            Class::Whitespace => {
                if ignore_whitespace {
                    whitespace_removed += 1;
                } else {
                    bad_chars += 1;
                    push_problem(
                        &mut problems,
                        Problem::at(
                            format!(
                                "whitespace ({}) at position {} — allowed only when 'ignore_whitespace' is on",
                                describe_char(ch),
                                here.0
                            ),
                            here.0,
                            here.1,
                            here.2,
                        ),
                    );
                }
            }
            Class::Invalid => {
                bad_chars += 1;
                push_problem(
                    &mut problems,
                    Problem::at(
                        format!(
                            "invalid character {} at position {} — not in the Base64 alphabet",
                            describe_char(ch),
                            here.0
                        ),
                        here.0,
                        here.1,
                        here.2,
                    ),
                );
            }
        }
    }

    if whitespace_removed > 0 {
        notes.push(format!(
            "Ignored {whitespace_removed} whitespace character{} inside the data.",
            plural(whitespace_removed)
        ));
    }

    // ---- alphabet.
    let alphabet = match (variant, n_std > 0, n_url > 0) {
        (Variant::Standard, _, true) => {
            let (ch, p, l, c) = first_url.unwrap();
            problems.push(Problem::at(
                format!(
                    "{} at position {p} belongs to the URL-safe alphabet (RFC 4648 §5); \
'variant=standard' expects '+' and '/'",
                    describe_char(ch)
                ),
                p,
                l,
                c,
            ));
            "standard"
        }
        (Variant::Standard, _, false) => "standard",
        (Variant::UrlSafe, true, _) => {
            let (ch, p, l, c) = first_std.unwrap();
            problems.push(Problem::at(
                format!(
                    "{} at position {p} belongs to the standard alphabet (RFC 4648 §4); \
'variant=url-safe' expects '-' and '_'",
                    describe_char(ch)
                ),
                p,
                l,
                c,
            ));
            "url-safe"
        }
        (Variant::UrlSafe, false, _) => "url-safe",
        (Variant::Auto, true, true) => {
            let (sc, sp, ..) = first_std.unwrap();
            let (uc, up, ..) = first_url.unwrap();
            problems.push(Problem::general(format!(
                "mixed alphabets: {} at position {sp} is standard Base64 while {} at position {up} \
is URL-safe Base64url — a string can only use one",
                describe_char(sc),
                describe_char(uc)
            )));
            "mixed"
        }
        (Variant::Auto, true, false) => "standard",
        (Variant::Auto, false, true) => "url-safe",
        (Variant::Auto, false, false) => "either",
    };

    // ---- padding placement + count.
    if let Some((p, d)) = data_after_pad {
        problems.push(Problem::general(format!(
            "the padding '=' at position {p} is followed by more Base64 data at position {d} — \
'=' may only appear at the very end"
        )));
    }
    if n_pad > 2 {
        problems.push(Problem::general(format!(
            "found {n_pad} '=' characters; Base64 padding is at most 2"
        )));
    }

    let d = data.chars().count();
    let rem = d % 4;
    let expected_pad = if rem == 0 { 0 } else { 4 - rem };
    if rem == 1 {
        problems.push(Problem::general(format!(
            "{d} data characters is not a valid Base64 length: a group of 1 leftover character \
encodes no bytes (a valid length leaves 0, 2, or 3 over after dividing by 4)"
        )));
    } else if n_pad > 0 && n_pad != expected_pad && n_pad <= 2 {
        problems.push(Problem::general(format!(
            "wrong padding: {d} data characters need {expected_pad} '=' at the end, found {n_pad}"
        )));
    }

    match pad_policy {
        Padding::Required if rem != 1 && n_pad != expected_pad && n_pad == 0 => {
            problems.push(Problem::general(format!(
                "padding is missing: 'padding=required' expects {expected_pad} '=' at the end so \
the total length is a multiple of 4"
            )));
        }
        Padding::Forbidden if n_pad > 0 => {
            problems.push(Problem::general(format!(
                "padding is not allowed: 'padding=forbidden' expects an unpadded string, found \
{n_pad} '='"
            )));
        }
        _ => {}
    }

    // ---- line length (checked against the original text, not the cleaned data).
    let mut long_lines: Vec<(usize, usize)> = Vec::new();
    if max_line_length > 0 {
        for (i, raw_line) in trimmed.split('\n').enumerate() {
            let len = raw_line.trim_end_matches('\r').chars().count();
            if len > max_line_length as usize {
                long_lines.push((i + 1, len));
            }
        }
        for (n, len) in long_lines.iter().take(3) {
            problems.push(Problem::general(format!(
                "line {n} is {len} characters, longer than the {max_line_length}-character limit"
            )));
        }
        if long_lines.len() > 3 {
            problems.push(Problem::general(format!(
                "…and {} more line{} over the limit",
                long_lines.len() - 3,
                plural(long_lines.len() - 3)
            )));
        }
    }

    // ---- canonical trailing bits + decode, only when everything else is clean.
    let valid = problems.is_empty();
    let mut canonical = true;
    let mut fixed_tail: Option<char> = None;
    if rem == 2 || rem == 3 {
        let last = data.chars().last().unwrap();
        let v = sym_value(last).unwrap_or(0);
        let mask = if rem == 2 { 0x0F } else { 0x03 };
        if v & mask != 0 {
            canonical = false;
            fixed_tail = Some(sym_char(v & !mask));
        }
    }
    if valid && !canonical {
        let last = data.chars().last().unwrap();
        warnings.push(format!(
            "non-canonical: the final character '{}' has unused trailing bits set (RFC 4648 §3.5). \
Strict decoders reject it, and re-encoding the decoded bytes yields '{}' instead.",
            display_char(last, alphabet),
            display_char(fixed_tail.unwrap(), alphabet)
        ));
    }

    let decoded = if valid { Some(decode(&data)) } else { None };
    let decoded_len = decoded.as_ref().map(|b| b.len()).unwrap_or(d * 6 / 8);

    // ---- suggested repair for a fixable invalid string.
    let suggestion = if valid || rem == 1 {
        None
    } else {
        let target = match (variant, alphabet) {
            (Variant::UrlSafe, _) | (Variant::Auto, "url-safe") => "url-safe",
            _ => "standard",
        };
        let mut s: String = data
            .chars()
            .map(|c| match (c, target) {
                ('+', "url-safe") => '-',
                ('/', "url-safe") => '_',
                _ => c,
            })
            .collect();
        // Keep the caller's padding intent: always pad when it is required,
        // never when it is forbidden, and otherwise only if they tried to pad.
        if pad_policy == Padding::Required || (pad_policy == Padding::Optional && n_pad > 0) {
            for _ in 0..expected_pad {
                s.push('=');
            }
        }
        if s == payload {
            None
        } else {
            Some(s)
        }
    };

    let hidden = bad_chars.saturating_sub(problems.iter().filter(|p| p.position.is_some()).count());

    let report = Report {
        valid,
        alphabet,
        variant,
        pad_policy,
        data_chars: d,
        pad_chars: n_pad,
        input_chars: input.chars().count(),
        whitespace_removed,
        decoded_len,
        canonical,
        decoded,
        problems,
        hidden,
        notes,
        warnings,
        suggestion,
    };

    Ok(if as_json {
        render_json(&report)
    } else {
        render_text(&report)
    })
}

/// Everything the renderers need, gathered in one place.
struct Report {
    valid: bool,
    alphabet: &'static str,
    variant: Variant,
    pad_policy: Padding,
    data_chars: usize,
    pad_chars: usize,
    input_chars: usize,
    whitespace_removed: usize,
    decoded_len: usize,
    canonical: bool,
    decoded: Option<Vec<u8>>,
    problems: Vec<Problem>,
    hidden: usize,
    notes: Vec<String>,
    warnings: Vec<String>,
    suggestion: Option<String>,
}

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    if r.valid {
        out.push_str(&format!("VALID — {}\n", alphabet_headline(r.alphabet)));
    } else {
        let n = r.problems.len();
        out.push_str(&format!("INVALID — {n} problem{}\n", plural(n)));
    }

    if !r.problems.is_empty() {
        out.push('\n');
        for (i, p) in r.problems.iter().enumerate() {
            match (p.line, p.column) {
                (Some(l), Some(c)) => out.push_str(&format!(
                    "{}. {} (line {l}, column {c})\n",
                    i + 1,
                    p.message
                )),
                _ => out.push_str(&format!("{}. {}\n", i + 1, p.message)),
            }
        }
        if r.hidden > 0 {
            out.push_str(&format!(
                "…and {} more invalid character{} not listed.\n",
                r.hidden,
                plural(r.hidden)
            ));
        }
    }

    out.push('\n');
    out.push_str(&format!("Alphabet:     {}\n", alphabet_detail(r.alphabet)));
    out.push_str(&format!(
        "Characters:   {} data + {} padding ({} in, padding policy: {})\n",
        r.data_chars,
        r.pad_chars,
        r.input_chars,
        padding_word(r.pad_policy)
    ));
    if r.valid {
        out.push_str(&format!(
            "Decoded size: {} byte{}\n",
            r.decoded_len,
            plural(r.decoded_len)
        ));
        out.push_str(&format!(
            "Canonical:    {}\n",
            if r.canonical {
                "yes"
            } else {
                "no — unused trailing bits are set"
            }
        ));
        if let Some(bytes) = &r.decoded {
            out.push_str(&format!("Content:      {}\n", describe_payload(bytes)));
        }
    } else {
        out.push_str(&format!(
            "Decoded size: {} byte{} if the problems above are fixed\n",
            r.decoded_len,
            plural(r.decoded_len)
        ));
    }

    if let Some(s) = &r.suggestion {
        out.push_str("\nSuggested fix:\n");
        out.push_str(s);
        out.push('\n');
    }
    for w in &r.warnings {
        out.push_str(&format!("\nWarning: {w}\n"));
    }
    if !r.notes.is_empty() {
        out.push_str("\nNotes:\n");
        for n in &r.notes {
            out.push_str(&format!("- {n}\n"));
        }
    }
    out
}

fn render_json(r: &Report) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"valid\": {},\n", r.valid));
    out.push_str(&format!("  \"alphabet\": \"{}\",\n", r.alphabet));
    out.push_str(&format!(
        "  \"variant\": \"{}\",\n",
        variant_name(r.variant)
    ));
    out.push_str(&format!(
        "  \"padding_policy\": \"{}\",\n",
        padding_name(r.pad_policy)
    ));
    out.push_str(&format!("  \"data_characters\": {},\n", r.data_chars));
    out.push_str(&format!("  \"padding_characters\": {},\n", r.pad_chars));
    out.push_str(&format!("  \"input_characters\": {},\n", r.input_chars));
    out.push_str(&format!(
        "  \"whitespace_ignored\": {},\n",
        r.whitespace_removed
    ));
    out.push_str(&format!("  \"decoded_bytes\": {},\n", r.decoded_len));
    out.push_str(&format!("  \"canonical\": {},\n", r.canonical));
    if let Some(bytes) = &r.decoded {
        out.push_str(&format!(
            "  \"content\": \"{}\",\n",
            esc(&describe_payload(bytes))
        ));
    }
    out.push_str("  \"problems\": [");
    if r.problems.is_empty() {
        out.push_str("],\n");
    } else {
        out.push('\n');
        for (i, p) in r.problems.iter().enumerate() {
            out.push_str(&format!("    {{ \"message\": \"{}\"", esc(&p.message)));
            if let Some(v) = p.position {
                out.push_str(&format!(", \"position\": {v}"));
            }
            if let Some(v) = p.line {
                out.push_str(&format!(", \"line\": {v}"));
            }
            if let Some(v) = p.column {
                out.push_str(&format!(", \"column\": {v}"));
            }
            out.push_str(" }");
            out.push_str(if i + 1 == r.problems.len() {
                "\n"
            } else {
                ",\n"
            });
        }
        out.push_str("  ],\n");
    }
    out.push_str(&format!("  \"problems_not_listed\": {},\n", r.hidden));
    out.push_str(&format!(
        "  \"warnings\": {},\n",
        json_str_array(&r.warnings)
    ));
    out.push_str(&format!("  \"notes\": {}", json_str_array(&r.notes)));
    match &r.suggestion {
        Some(s) => out.push_str(&format!(",\n  \"suggestion\": \"{}\"\n", esc(s))),
        None => out.push('\n'),
    }
    out.push('}');
    out
}

// ---------------------------------------------------------------- helpers

enum Class {
    Shared,
    Standard,
    UrlSafe,
    Pad,
    Whitespace,
    Invalid,
}

fn classify(c: char) -> Class {
    match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' => Class::Shared,
        '+' | '/' => Class::Standard,
        '-' | '_' => Class::UrlSafe,
        '=' => Class::Pad,
        c if c.is_ascii_whitespace() => Class::Whitespace,
        _ => Class::Invalid,
    }
}

fn push_problem(problems: &mut Vec<Problem>, p: Problem) {
    if problems.len() < MAX_PROBLEMS {
        problems.push(p);
    }
}

fn parse_variant(s: &str) -> Result<Variant, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(Variant::Auto),
        "standard" | "base64" => Ok(Variant::Standard),
        "url-safe" | "url_safe" | "urlsafe" | "base64url" => Ok(Variant::UrlSafe),
        other => Err(format!(
            "invalid variant {other:?}: expected 'auto', 'standard', or 'url-safe'"
        )),
    }
}

fn parse_padding(s: &str) -> Result<Padding, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "optional" => Ok(Padding::Optional),
        "required" => Ok(Padding::Required),
        "forbidden" | "none" => Ok(Padding::Forbidden),
        other => Err(format!(
            "invalid padding {other:?}: expected 'optional', 'required', or 'forbidden'"
        )),
    }
}

fn parse_output(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(false),
        "json" => Ok(true),
        other => Err(format!(
            "invalid output {other:?}: expected 'text' or 'json'"
        )),
    }
}

fn variant_name(v: Variant) -> &'static str {
    match v {
        Variant::Auto => "auto",
        Variant::Standard => "standard",
        Variant::UrlSafe => "url-safe",
    }
}

fn padding_name(p: Padding) -> &'static str {
    match p {
        Padding::Optional => "optional",
        Padding::Required => "required",
        Padding::Forbidden => "forbidden",
    }
}

fn padding_word(p: Padding) -> &'static str {
    match p {
        Padding::Optional => "optional",
        Padding::Required => "required",
        Padding::Forbidden => "forbidden",
    }
}

fn alphabet_headline(a: &str) -> &'static str {
    match a {
        "standard" => "standard Base64 (RFC 4648 §4)",
        "url-safe" => "URL-safe Base64url (RFC 4648 §5)",
        _ => "Base64 (standard or URL-safe — no distinguishing characters)",
    }
}

fn alphabet_detail(a: &str) -> &'static str {
    match a {
        "standard" => "standard — uses '+' and '/'",
        "url-safe" => "url-safe — uses '-' and '_'",
        "mixed" => "mixed — both '+'/'/' and '-'/'_' are present",
        _ => "either — no '+', '/', '-' or '_' to tell them apart",
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// `'x' (U+0078)` — quoted when printable, code point always.
fn describe_char(c: char) -> String {
    if c == ' ' {
        return "a space (U+0020)".into();
    }
    if c == '\t' {
        return "a tab (U+0009)".into();
    }
    if c == '\n' {
        return "a line feed (U+000A)".into();
    }
    if c == '\r' {
        return "a carriage return (U+000D)".into();
    }
    if c.is_control() {
        return format!("a control character (U+{:04X})", c as u32);
    }
    format!("'{c}' (U+{:04X})", c as u32)
}

/// Render a data character in the alphabet the string actually uses.
fn display_char(c: char, alphabet: &str) -> char {
    match (c, alphabet) {
        ('+', "url-safe") => '-',
        ('/', "url-safe") => '_',
        _ => c,
    }
}

/// Case-insensitive substring search over the first `limit` bytes.
fn find_ci(hay: &str, needle: &str, limit: usize) -> Option<usize> {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    let end = hay.len().min(limit);
    if n.is_empty() || end < n.len() {
        return None;
    }
    (0..=end - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// 1-based line and 0-based column immediately before byte offset `at`.
fn line_col_at(s: &str, at: usize) -> (usize, usize) {
    let head = &s[..at];
    let line = 1 + head.matches('\n').count();
    let column = match head.rfind('\n') {
        Some(i) => head[i + 1..].chars().count(),
        None => head.chars().count(),
    };
    (line, column)
}

fn sym_value(c: char) -> Option<u8> {
    match c {
        'A'..='Z' => Some(c as u8 - b'A'),
        'a'..='z' => Some(c as u8 - b'a' + 26),
        '0'..='9' => Some(c as u8 - b'0' + 52),
        '+' => Some(62),
        '/' => Some(63),
        _ => None,
    }
}

fn sym_char(v: u8) -> char {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    ALPHABET[(v & 63) as usize] as char
}

/// Decode already-validated, standard-alphabet data characters (no padding).
fn decode(data: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 4 * 3 + 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for c in data.chars() {
        let v = match sym_value(c) {
            Some(v) => v as u32,
            None => continue,
        };
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    out
}

/// "text — \"Hello\"" or "binary — PNG image (89 50 4e 47 …)".
fn describe_payload(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "empty — decodes to 0 bytes".into();
    }
    if let Ok(s) = core_str(bytes) {
        if s.chars()
            .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r'))
        {
            let mut preview: String = s
                .chars()
                .take(TEXT_PREVIEW_CHARS)
                .map(|c| match c {
                    '\n' => '␊',
                    '\r' => '␍',
                    '\t' => '␉',
                    c => c,
                })
                .collect();
            if s.chars().count() > TEXT_PREVIEW_CHARS {
                preview.push('…');
            }
            return format!("text — \"{preview}\"");
        }
    }
    let kind = sniff(bytes).unwrap_or("binary data");
    let hex: Vec<String> = bytes
        .iter()
        .take(HEX_PREVIEW_BYTES)
        .map(|b| format!("{b:02x}"))
        .collect();
    let ellipsis = if bytes.len() > HEX_PREVIEW_BYTES {
        " …"
    } else {
        ""
    };
    format!("binary — {kind} ({}{ellipsis})", hex.join(" "))
}

fn core_str(bytes: &[u8]) -> Result<&str, ()> {
    std::str::from_utf8(bytes).map_err(|_| ())
}

/// Best-effort magic-byte sniff for the containers people paste as Base64.
fn sniff(b: &[u8]) -> Option<&'static str> {
    let starts = |p: &[u8]| b.len() >= p.len() && &b[..p.len()] == p;
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("PNG image");
    }
    if starts(b"\xFF\xD8\xFF") {
        return Some("JPEG image");
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some("GIF image");
    }
    if starts(b"BM") {
        return Some("BMP image");
    }
    if starts(b"%PDF-") {
        return Some("PDF document");
    }
    if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") {
        return Some("ZIP archive (or a ZIP-based file such as .docx/.xlsx/.jar)");
    }
    if starts(b"\x1F\x8B") {
        return Some("gzip data");
    }
    if starts(b"BZh") {
        return Some("bzip2 data");
    }
    if starts(b"7z\xBC\xAF\x27\x1C") {
        return Some("7-Zip archive");
    }
    if starts(b"Rar!") {
        return Some("RAR archive");
    }
    if starts(b"OggS") {
        return Some("Ogg media");
    }
    if starts(b"fLaC") {
        return Some("FLAC audio");
    }
    if starts(b"ID3") || starts(b"\xFF\xFB") {
        return Some("MP3 audio");
    }
    if b.len() >= 12 && &b[..4] == b"RIFF" {
        if &b[8..12] == b"WEBP" {
            return Some("WebP image");
        }
        if &b[8..12] == b"WAVE" {
            return Some("WAV audio");
        }
        return Some("RIFF container");
    }
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return Some("MP4/QuickTime media");
    }
    if starts(b"\x7FELF") {
        return Some("ELF executable");
    }
    if starts(b"SQLite format 3\0") {
        return Some("SQLite database");
    }
    if starts(b"\x00\x01\x00\x00\x00") || starts(b"OTTO") || starts(b"wOFF") || starts(b"wOF2") {
        return Some("font file");
    }
    None
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".into();
    }
    let inner: Vec<String> = items.iter().map(|s| format!("\"{}\"", esc(s))).collect();
    format!("[{}]", inner.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(input: &str) -> String {
        validate(input, "auto", "optional", true, 0, "text").unwrap()
    }

    #[test]
    fn valid_padded_standard_string() {
        let out = v("SGVsbG8sIHdvcmxkIQ==");
        assert!(
            out.starts_with(
                "VALID — Base64 (standard or URL-safe — no distinguishing characters)\n"
            ),
            "{out}"
        );
        assert!(out.contains("Decoded size: 13 bytes"), "{out}");
        assert!(
            out.contains("Content:      text — \"Hello, world!\""),
            "{out}"
        );
        assert!(out.contains("Canonical:    yes"), "{out}");
    }

    #[test]
    fn valid_unpadded_url_safe_string() {
        let out = v("SGVsbG8_d29ybGQ");
        assert!(
            out.starts_with("VALID — URL-safe Base64url (RFC 4648 §5)\n"),
            "{out}"
        );
        assert!(out.contains("url-safe — uses '-' and '_'"), "{out}");
    }

    #[test]
    fn invalid_character_is_reported_with_position() {
        let out = v("SGVsbG8sIHdvcmxkIQ=?");
        assert!(out.starts_with("INVALID — "), "{out}");
        assert!(
            out.contains("invalid character '?' (U+003F) at position 20"),
            "{out}"
        );
        assert!(out.contains("(line 1, column 20)"), "{out}");
    }

    #[test]
    fn missing_padding_only_fails_when_required() {
        let ok = v("SGVsbG8sIHdvcmxkIQ");
        assert!(ok.starts_with("VALID"), "{ok}");
        let bad = validate("SGVsbG8sIHdvcmxkIQ", "auto", "required", true, 0, "text").unwrap();
        assert!(bad.contains("padding is missing"), "{bad}");
        assert!(
            bad.contains("Suggested fix:\nSGVsbG8sIHdvcmxkIQ==\n"),
            "{bad}"
        );
    }

    #[test]
    fn padding_forbidden_rejects_equals() {
        let out = validate("SGVsbG8sIHdvcmxkIQ==", "auto", "forbidden", true, 0, "text").unwrap();
        assert!(out.contains("padding is not allowed"), "{out}");
        assert!(
            out.contains("Suggested fix:\nSGVsbG8sIHdvcmxkIQ\n"),
            "{out}"
        );
    }

    #[test]
    fn wrong_number_of_equals_is_reported() {
        let out = v("SGVsbG8=");
        assert!(out.starts_with("VALID"), "{out}");
        let out = v("SGVsbG8==");
        assert!(
            out.contains("wrong padding: 7 data characters need 1 '=' at the end, found 2"),
            "{out}"
        );
    }

    #[test]
    fn padding_in_the_middle_is_reported() {
        let out = v("SGVs=G8sIHdvcmxkIQ==");
        assert!(out.contains("is followed by more Base64 data"), "{out}");
    }

    #[test]
    fn remainder_of_one_has_no_suggestion() {
        let out = v("SGVsbG8sIHdvcmxkI");
        assert!(out.contains("a group of 1 leftover character"), "{out}");
        assert!(!out.contains("Suggested fix"), "{out}");
    }

    #[test]
    fn variant_standard_rejects_url_safe_characters() {
        let out = validate("SGVsbG8_d29ybGQ", "standard", "optional", true, 0, "text").unwrap();
        assert!(out.contains("belongs to the URL-safe alphabet"), "{out}");
        assert!(out.contains("Suggested fix:\nSGVsbG8/d29ybGQ\n"), "{out}");
    }

    #[test]
    fn variant_url_safe_rejects_standard_characters() {
        let out = validate("SGVsbG8/d29ybGQ", "url-safe", "optional", true, 0, "text").unwrap();
        assert!(out.contains("belongs to the standard alphabet"), "{out}");
        assert!(out.contains("Suggested fix:\nSGVsbG8_d29ybGQ\n"), "{out}");
    }

    #[test]
    fn mixed_alphabets_are_reported_under_auto() {
        let out = v("SGVs-G8/d29ybGQ");
        assert!(out.contains("mixed alphabets"), "{out}");
        assert!(out.contains("mixed — both"), "{out}");
    }

    #[test]
    fn whitespace_is_ignored_by_default_and_flagged_when_strict() {
        let ok = v("SGVsbG8s\nIHdvcmxkIQ==");
        assert!(ok.starts_with("VALID"), "{ok}");
        assert!(
            ok.contains("Ignored 1 whitespace character inside the data."),
            "{ok}"
        );
        let strict = validate(
            "SGVsbG8s\nIHdvcmxkIQ==",
            "auto",
            "optional",
            false,
            0,
            "text",
        )
        .unwrap();
        assert!(
            strict.contains("whitespace (a line feed (U+000A))"),
            "{strict}"
        );
        assert!(strict.contains("(line 1, column 9)"), "{strict}");
    }

    #[test]
    fn line_length_limit_is_enforced() {
        let long = "QUJDRA==";
        let ok = validate(long, "auto", "optional", true, 76, "text").unwrap();
        assert!(ok.starts_with("VALID"), "{ok}");
        let out = validate(long, "auto", "optional", true, 4, "text").unwrap();
        assert!(
            out.contains("line 1 is 8 characters, longer than the 4-character limit"),
            "{out}"
        );
    }

    #[test]
    fn non_canonical_trailing_bits_warn_but_stay_valid() {
        let out = v("QUJDRB==");
        assert!(out.starts_with("VALID"), "{out}");
        assert!(out.contains("Canonical:    no"), "{out}");
        assert!(out.contains("yields 'A' instead"), "{out}");
    }

    #[test]
    fn data_uri_prefix_is_stripped_and_noted() {
        let out = v("data:text/plain;base64,SGVsbG8=");
        assert!(out.starts_with("VALID"), "{out}");
        assert!(out.contains("Detected a data: URI"), "{out}");
        assert!(out.contains("Content:      text — \"Hello\""), "{out}");
    }

    #[test]
    fn wrapping_quotes_are_stripped() {
        let out = v("\"SGVsbG8=\"");
        assert!(out.starts_with("VALID"), "{out}");
        assert!(out.contains("Removed the wrapping double quotes."), "{out}");
    }

    #[test]
    fn binary_payloads_are_sniffed() {
        let out = v("iVBORw0KGgoAAAANSUhEUg==");
        assert!(out.contains("binary — PNG image (89 50 4e 47"), "{out}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = validate("SGVsbG8=", "auto", "optional", true, 0, "json").unwrap();
        assert!(out.contains("\"valid\": true"), "{out}");
        assert!(out.contains("\"decoded_bytes\": 5"), "{out}");
        assert!(out.contains("\"alphabet\": \"either\""), "{out}");
        assert!(out.contains("\"problems\": [],"), "{out}");
    }

    #[test]
    fn json_output_lists_problems() {
        let out = validate("SGVsbG8sI!Q==", "auto", "optional", true, 0, "json").unwrap();
        assert!(out.contains("\"valid\": false"), "{out}");
        assert!(out.contains("\"position\": 10"), "{out}");
        assert!(out.contains("\"suggestion\": \""), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = validate("   ", "auto", "optional", true, 0, "text").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn unknown_option_values_are_errors() {
        assert!(validate("QQ==", "sideways", "optional", true, 0, "text")
            .unwrap_err()
            .contains("expected 'auto', 'standard', or 'url-safe'"));
        assert!(validate("QQ==", "auto", "maybe", true, 0, "text")
            .unwrap_err()
            .contains("expected 'optional', 'required', or 'forbidden'"));
        assert!(validate("QQ==", "auto", "optional", true, 0, "yaml")
            .unwrap_err()
            .contains("expected 'text' or 'json'"));
        assert!(validate("QQ==", "auto", "optional", true, 9999, "text")
            .unwrap_err()
            .contains("expected 0 (no check) to 998"));
        assert!(validate("QQ==", "auto", "optional", true, -1, "text")
            .unwrap_err()
            .contains("expected 0 (no check) to 998"));
    }

    #[test]
    fn oversize_input_is_rejected() {
        let big = "A".repeat(MAX_INPUT_BYTES + 1);
        let err = validate(&big, "auto", "optional", true, 0, "text").unwrap_err();
        assert!(err.contains("input is too large"), "{err}");
    }
}
