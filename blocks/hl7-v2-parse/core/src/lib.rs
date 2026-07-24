//! hl7-v2-parse core — parse a pipe-delimited HL7 v2.x message into structured
//! segments, fields, components and subcomponents. Pure compute — no I/O, no
//! wafer/wasm-bindgen deps. Shared by the chat skill block, the CLI, and the web page.
//!
//! HL7 v2 layout: a message is a set of SEGMENTS separated by carriage returns
//! (this parser also accepts `\n`). Each segment starts with a 3-character name
//! (MSH, PID, OBX, …) followed by delimited FIELDS. The delimiters are declared
//! by the MSH segment itself:
//!   MSH-1 = the field separator (the character right after `MSH`, usually `|`)
//!   MSH-2 = the encoding characters: component `^`, repetition `~`, escape `\`,
//!           subcomponent `&` (the default `^~\&`).
//! A field may repeat (`~`), split into components (`^`), and a component into
//! subcomponents (`&`). The MSH segment is special: MSH-1 is the separator itself
//! and MSH-2 is the raw encoding characters, so field numbering after MSH is
//! offset by one — this parser reproduces that offset faithfully.

use serde_json::{json, Map, Value};

/// Output rendering: nested JSON (default) or a flat CSV leaf table.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Json,
    Csv,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Output::Json),
            "csv" => Ok(Output::Csv),
            other => Err(format!("unknown output '{other}' (use 'json' or 'csv')")),
        }
    }
}

/// The five HL7 encoding characters, read from the MSH segment (or defaulted).
#[derive(Clone, Copy)]
struct Enc {
    field: char,
    component: char,
    repetition: char,
    escape: char,
    subcomponent: char,
}

impl Default for Enc {
    fn default() -> Enc {
        Enc { field: '|', component: '^', repetition: '~', escape: '\\', subcomponent: '&' }
    }
}

/// One raw segment: its 3+ char name, whether it is the header MSH, and its
/// fields as raw strings indexed from 1 (fields[0] is field #1).
struct RawSeg {
    name: String,
    is_msh: bool,
    fields: Vec<String>,
}

/// String-typed entry point shared by the chat block, CLI, and web page.
///
/// - `output`: `json` (default) or `csv`.
/// - `include_descriptions`: attach human-readable segment/field names.
/// - `unescape`: decode HL7 escape sequences (`\F\ \S\ \T\ \R\ \E\ \Xhh\ \.br\`)
///   into their literal characters.
pub fn run(
    data: &str,
    output: &str,
    include_descriptions: bool,
    unescape: bool,
) -> Result<String, String> {
    let out = Output::parse(output)?;
    let (segs, enc) = parse(data)?;
    match out {
        Output::Json => Ok(render_json(&segs, enc, include_descriptions, unescape)),
        Output::Csv => Ok(render_csv(&segs, enc, include_descriptions, unescape)),
    }
}

/// Split the raw message into segments and determine the encoding characters.
fn parse(data: &str) -> Result<(Vec<RawSeg>, Enc), String> {
    // Segments are separated by \r, \n, or \r\n; blank lines are ignored.
    let lines: Vec<&str> = data
        .split(['\r', '\n'])
        .map(|l| l.trim_matches(|c| c == ' ' || c == '\t'))
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(
            "no HL7 segments found; expected pipe-delimited lines like 'MSH|^~\\&|APP|FAC|...'"
                .into(),
        );
    }

    // Determine delimiters from the first header segment (MSH/BHS/FHS carry them).
    let enc = header_enc(&lines);

    let mut segs = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let name: String = line.chars().take(3).collect();
        let is_header = matches!(name.as_str(), "MSH" | "BHS" | "FHS");
        if is_header {
            if line.chars().count() < 4 {
                return Err(format!(
                    "segment {} ('{name}') is too short to contain the field separator and encoding characters",
                    i + 1
                ));
            }
            // Field #1 is the separator itself; field #2 is the raw encoding
            // characters; the rest split normally after them.
            let after: String = line.chars().skip(3).collect(); // e.g. "|^~\&|A|B"
            let mut parts: Vec<String> = after.split(enc.field).map(|s| s.to_string()).collect();
            // parts[0] is the empty string before the first separator.
            let mut fields = vec![enc.field.to_string()]; // field #1 = separator
            if parts.len() > 1 {
                fields.extend(parts.drain(1..)); // field #2 = encoding chars, then #3..
            }
            segs.push(RawSeg { name, is_msh: true, fields });
        } else {
            let mut parts: Vec<String> = line.split(enc.field).map(|s| s.to_string()).collect();
            let seg_name = parts.remove(0);
            if seg_name.is_empty() {
                return Err(format!(
                    "segment {} has no segment name (a line may not start with the field separator '{}')",
                    i + 1,
                    enc.field
                ));
            }
            segs.push(RawSeg { name: seg_name, is_msh: false, fields: parts });
        }
    }
    Ok((segs, enc))
}

/// Read the encoding characters from the first MSH/BHS/FHS header, else default.
fn header_enc(lines: &[&str]) -> Enc {
    for line in lines {
        let name: String = line.chars().take(3).collect();
        if matches!(name.as_str(), "MSH" | "BHS" | "FHS") {
            let chars: Vec<char> = line.chars().collect();
            if chars.len() >= 4 {
                let field = chars[3];
                // Encoding chars are the run after the field separator up to the
                // next field separator: chars[4..] until `field` again.
                let enc_chars: Vec<char> =
                    chars[4..].iter().take_while(|&&c| c != field).copied().collect();
                let get = |i: usize, d: char| enc_chars.get(i).copied().unwrap_or(d);
                return Enc {
                    field,
                    component: get(0, '^'),
                    repetition: get(1, '~'),
                    escape: get(2, '\\'),
                    subcomponent: get(3, '&'),
                };
            }
        }
    }
    Enc::default()
}

// ---------------------------------------------------------------------------
// HL7 escape-sequence decoding
// ---------------------------------------------------------------------------

/// Decode HL7 escape sequences delimited by the escape character. Recognises the
/// standard `\F\ \S\ \T\ \R\ \E\`, hex `\Xhh..\`, and `\.br\` (line break).
/// Formatting/unknown sequences (`\H\ \N\ \Cxxyy\ …`) are dropped where they carry
/// no textual content; anything unrecognised is left verbatim so no data is lost.
fn decode(s: &str, enc: Enc) -> String {
    if !s.contains(enc.escape) {
        return s.to_string();
    }
    let esc = enc.escape;
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == esc {
            // Find the closing escape char.
            if let Some(rel) = chars[i + 1..].iter().position(|&c| c == esc) {
                let seq: String = chars[i + 1..i + 1 + rel].iter().collect();
                match decode_seq(&seq, enc) {
                    Some(text) => out.push_str(&text),
                    None => {
                        // Unknown sequence: keep it verbatim (delimiters included).
                        out.push(esc);
                        out.push_str(&seq);
                        out.push(esc);
                    }
                }
                i += rel + 2; // past the closing escape
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn decode_seq(seq: &str, enc: Enc) -> Option<String> {
    match seq {
        "F" => Some(enc.field.to_string()),
        "S" => Some(enc.component.to_string()),
        "T" => Some(enc.subcomponent.to_string()),
        "R" => Some(enc.repetition.to_string()),
        "E" => Some(enc.escape.to_string()),
        ".br" => Some("\n".to_string()),
        "H" | "N" => Some(String::new()), // highlight on/off — no text
        _ => {
            if let Some(hex) = seq.strip_prefix('X') {
                // \Xhh..\ — hex bytes → UTF-8 text.
                if !hex.is_empty() && hex.len() % 2 == 0 && hex.chars().all(|c| c.is_ascii_hexdigit())
                {
                    let bytes: Vec<u8> = (0..hex.len())
                        .step_by(2)
                        .map(|k| u8::from_str_radix(&hex[k..k + 2], 16).unwrap())
                        .collect();
                    return Some(String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            None
        }
    }
}

fn maybe_decode(s: &str, enc: Enc, unescape: bool) -> String {
    if unescape {
        decode(s, enc)
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// JSON rendering (nested hierarchy)
// ---------------------------------------------------------------------------

fn render_json(segs: &[RawSeg], enc: Enc, desc: bool, unescape: bool) -> String {
    let arr: Vec<Value> = segs.iter().map(|s| segment_json(s, enc, desc, unescape)).collect();
    serde_json::to_string_pretty(&Value::Array(arr)).unwrap()
}

fn segment_json(seg: &RawSeg, enc: Enc, desc: bool, unescape: bool) -> Value {
    let mut m = Map::new();
    m.insert("segment".into(), json!(seg.name));
    if desc {
        if let Some(d) = segment_desc(&seg.name) {
            m.insert("description".into(), json!(d));
        }
    }
    let fields: Vec<Value> = seg
        .fields
        .iter()
        .enumerate()
        .map(|(k, raw)| {
            let idx = k + 1;
            // For a header segment, fields #1 and #2 are literal (separator +
            // encoding characters) and must not be split into components.
            let literal = seg.is_msh && idx <= 2;
            field_json(&seg.name, idx, raw, enc, desc, unescape, literal)
        })
        .collect();
    m.insert("fields".into(), Value::Array(fields));
    Value::Object(m)
}

fn field_json(
    seg: &str,
    idx: usize,
    raw: &str,
    enc: Enc,
    desc: bool,
    unescape: bool,
    literal: bool,
) -> Value {
    let mut m = Map::new();
    m.insert("id".into(), json!(format!("{seg}.{idx}")));
    if literal {
        m.insert("value".into(), json!(raw));
    } else {
        let reps: Vec<&str> = raw.split(enc.repetition).collect();
        if reps.len() > 1 {
            m.insert("value".into(), json!(raw));
            let arr: Vec<Value> = reps.iter().map(|r| rep_json(r, enc, unescape)).collect();
            m.insert("repetitions".into(), Value::Array(arr));
        } else {
            let comps: Vec<&str> = raw.split(enc.component).collect();
            if comps.len() > 1 {
                m.insert("value".into(), json!(raw));
                let arr: Vec<Value> =
                    comps.iter().map(|c| component_json(c, enc, unescape)).collect();
                m.insert("components".into(), Value::Array(arr));
            } else {
                let subs: Vec<&str> = raw.split(enc.subcomponent).collect();
                if subs.len() > 1 {
                    m.insert("value".into(), json!(raw));
                    let arr: Vec<Value> =
                        subs.iter().map(|s| json!(maybe_decode(s, enc, unescape))).collect();
                    m.insert("subcomponents".into(), Value::Array(arr));
                } else {
                    m.insert("value".into(), json!(maybe_decode(raw, enc, unescape)));
                }
            }
        }
    }
    if desc {
        if let Some(d) = field_desc(seg, idx) {
            m.insert("description".into(), json!(d));
        }
    }
    Value::Object(m)
}

/// A repetition body: either a scalar string, or an object with components.
fn rep_json(rep: &str, enc: Enc, unescape: bool) -> Value {
    let comps: Vec<&str> = rep.split(enc.component).collect();
    if comps.len() > 1 {
        let mut m = Map::new();
        m.insert("value".into(), json!(rep));
        let arr: Vec<Value> = comps.iter().map(|c| component_json(c, enc, unescape)).collect();
        m.insert("components".into(), Value::Array(arr));
        Value::Object(m)
    } else {
        component_json(rep, enc, unescape)
    }
}

/// A component body: either a scalar string, or an object with subcomponents.
fn component_json(comp: &str, enc: Enc, unescape: bool) -> Value {
    let subs: Vec<&str> = comp.split(enc.subcomponent).collect();
    if subs.len() > 1 {
        let mut m = Map::new();
        m.insert("value".into(), json!(comp));
        let arr: Vec<Value> =
            subs.iter().map(|s| json!(maybe_decode(s, enc, unescape))).collect();
        m.insert("subcomponents".into(), Value::Array(arr));
        Value::Object(m)
    } else {
        json!(maybe_decode(comp, enc, unescape))
    }
}

// ---------------------------------------------------------------------------
// CSV rendering (flat leaf table)
// ---------------------------------------------------------------------------

fn render_csv(segs: &[RawSeg], enc: Enc, desc: bool, unescape: bool) -> String {
    let mut out = String::new();
    // Header row.
    out.push_str("Segment,Location,Value");
    if desc {
        out.push_str(",Description");
    }
    out.push('\n');

    for seg in segs {
        for (k, raw) in seg.fields.iter().enumerate() {
            let idx = k + 1;
            let literal = seg.is_msh && idx <= 2;
            if literal {
                push_row(&mut out, &seg.name, &format!("{}.{}", seg.name, idx), raw, desc, idx);
                continue;
            }
            let reps: Vec<&str> = raw.split(enc.repetition).collect();
            let multi_rep = reps.len() > 1;
            for (ri, rep) in reps.iter().enumerate() {
                let rep_sfx = if multi_rep { format!("[{}]", ri + 1) } else { String::new() };
                let comps: Vec<&str> = rep.split(enc.component).collect();
                let multi_comp = comps.len() > 1;
                for (ci, comp) in comps.iter().enumerate() {
                    let comp_sfx = if multi_comp { format!(".{}", ci + 1) } else { String::new() };
                    let subs: Vec<&str> = comp.split(enc.subcomponent).collect();
                    let multi_sub = subs.len() > 1;
                    for (si, sub) in subs.iter().enumerate() {
                        let sub_sfx =
                            if multi_sub { format!(".{}", si + 1) } else { String::new() };
                        let val = maybe_decode(sub, enc, unescape);
                        if val.is_empty() {
                            continue;
                        }
                        let loc = format!("{}.{}{}{}{}", seg.name, idx, rep_sfx, comp_sfx, sub_sfx);
                        push_row(&mut out, &seg.name, &loc, &val, desc, idx);
                    }
                }
            }
        }
    }
    out
}

fn push_row(out: &mut String, seg: &str, loc: &str, value: &str, desc: bool, idx: usize) {
    out.push_str(&csv_field(seg));
    out.push(',');
    out.push_str(&csv_field(loc));
    out.push(',');
    out.push_str(&csv_field(value));
    if desc {
        out.push(',');
        out.push_str(&csv_field(field_desc(seg, idx).unwrap_or("")));
    }
    out.push('\n');
}

/// RFC-4180 CSV field escaping (comma delimiter).
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Description dictionaries (curated common segments/fields)
// ---------------------------------------------------------------------------

/// Human-readable name for a segment code. Z-segments are site-defined.
pub fn segment_desc(name: &str) -> Option<&'static str> {
    let d = match name {
        "MSH" => "Message Header",
        "EVN" => "Event Type",
        "PID" => "Patient Identification",
        "PD1" => "Patient Additional Demographic",
        "NK1" => "Next of Kin / Associated Parties",
        "PV1" => "Patient Visit",
        "PV2" => "Patient Visit - Additional Information",
        "ROL" => "Role",
        "DB1" => "Disability",
        "OBR" => "Observation Request",
        "OBX" => "Observation / Result",
        "NTE" => "Notes and Comments",
        "AL1" => "Patient Allergy Information",
        "DG1" => "Diagnosis",
        "DRG" => "Diagnosis Related Group",
        "PR1" => "Procedures",
        "GT1" => "Guarantor",
        "IN1" => "Insurance",
        "IN2" => "Insurance - Additional Information",
        "IN3" => "Insurance - Additional Information, Certification",
        "ACC" => "Accident",
        "UB1" => "UB82 Data",
        "UB2" => "UB92 Data",
        "ORC" => "Common Order",
        "RXO" => "Pharmacy/Treatment Order",
        "RXE" => "Pharmacy/Treatment Encoded Order",
        "RXD" => "Pharmacy/Treatment Dispense",
        "RXG" => "Pharmacy/Treatment Give",
        "RXA" => "Pharmacy/Treatment Administration",
        "RXR" => "Pharmacy/Treatment Route",
        "SPM" => "Specimen",
        "SAC" => "Specimen Container Detail",
        "MSA" => "Message Acknowledgment",
        "ERR" => "Error",
        "QRD" => "Query Definition",
        "QRF" => "Query Filter",
        "QPD" => "Query Parameter Definition",
        "RCP" => "Response Control Parameter",
        "SCH" => "Scheduling Activity Information",
        "AIS" => "Appointment Information - Service",
        "AIG" => "Appointment Information - General Resource",
        "AIL" => "Appointment Information - Location Resource",
        "AIP" => "Appointment Information - Personnel Resource",
        "RGS" => "Resource Group",
        "TXA" => "Transcription Document Header",
        "MRG" => "Merge Patient Information",
        "FHS" => "File Header",
        "FTS" => "File Trailer",
        "BHS" => "Batch Header",
        "BTS" => "Batch Trailer",
        _ => return None,
    };
    Some(d)
}

/// Human-readable name for a field position in a common segment. Curated for the
/// segments most tools surface (MSH, EVN, PID, PV1, OBR, OBX, MSA, NK1, AL1, DG1,
/// IN1, ORC); the full HL7 data dictionary is version-specific and out of scope.
pub fn field_desc(seg: &str, idx: usize) -> Option<&'static str> {
    let d = match (seg, idx) {
        ("MSH", 1) => "Field Separator",
        ("MSH", 2) => "Encoding Characters",
        ("MSH", 3) => "Sending Application",
        ("MSH", 4) => "Sending Facility",
        ("MSH", 5) => "Receiving Application",
        ("MSH", 6) => "Receiving Facility",
        ("MSH", 7) => "Date/Time of Message",
        ("MSH", 8) => "Security",
        ("MSH", 9) => "Message Type",
        ("MSH", 10) => "Message Control ID",
        ("MSH", 11) => "Processing ID",
        ("MSH", 12) => "Version ID",
        ("MSH", 13) => "Sequence Number",
        ("MSH", 14) => "Continuation Pointer",
        ("MSH", 15) => "Accept Acknowledgment Type",
        ("MSH", 16) => "Application Acknowledgment Type",
        ("MSH", 17) => "Country Code",
        ("MSH", 18) => "Character Set",
        ("MSH", 19) => "Principal Language of Message",

        ("EVN", 1) => "Event Type Code",
        ("EVN", 2) => "Recorded Date/Time",
        ("EVN", 3) => "Date/Time Planned Event",
        ("EVN", 4) => "Event Reason Code",
        ("EVN", 5) => "Operator ID",
        ("EVN", 6) => "Event Occurred",
        ("EVN", 7) => "Event Facility",

        ("PID", 1) => "Set ID - PID",
        ("PID", 2) => "Patient ID",
        ("PID", 3) => "Patient Identifier List",
        ("PID", 4) => "Alternate Patient ID",
        ("PID", 5) => "Patient Name",
        ("PID", 6) => "Mother's Maiden Name",
        ("PID", 7) => "Date/Time of Birth",
        ("PID", 8) => "Administrative Sex",
        ("PID", 9) => "Patient Alias",
        ("PID", 10) => "Race",
        ("PID", 11) => "Patient Address",
        ("PID", 12) => "County Code",
        ("PID", 13) => "Phone Number - Home",
        ("PID", 14) => "Phone Number - Business",
        ("PID", 15) => "Primary Language",
        ("PID", 16) => "Marital Status",
        ("PID", 17) => "Religion",
        ("PID", 18) => "Patient Account Number",
        ("PID", 19) => "SSN Number - Patient",
        ("PID", 20) => "Driver's License Number - Patient",
        ("PID", 21) => "Mother's Identifier",
        ("PID", 22) => "Ethnic Group",
        ("PID", 23) => "Birth Place",
        ("PID", 24) => "Multiple Birth Indicator",
        ("PID", 25) => "Birth Order",
        ("PID", 26) => "Citizenship",
        ("PID", 27) => "Veterans Military Status",
        ("PID", 28) => "Nationality",
        ("PID", 29) => "Patient Death Date and Time",
        ("PID", 30) => "Patient Death Indicator",

        ("PV1", 1) => "Set ID - PV1",
        ("PV1", 2) => "Patient Class",
        ("PV1", 3) => "Assigned Patient Location",
        ("PV1", 4) => "Admission Type",
        ("PV1", 5) => "Preadmit Number",
        ("PV1", 6) => "Prior Patient Location",
        ("PV1", 7) => "Attending Doctor",
        ("PV1", 8) => "Referring Doctor",
        ("PV1", 9) => "Consulting Doctor",
        ("PV1", 10) => "Hospital Service",
        ("PV1", 11) => "Temporary Location",
        ("PV1", 17) => "Admitting Doctor",
        ("PV1", 18) => "Patient Type",
        ("PV1", 19) => "Visit Number",
        ("PV1", 44) => "Admit Date/Time",
        ("PV1", 45) => "Discharge Date/Time",

        ("OBR", 1) => "Set ID - OBR",
        ("OBR", 2) => "Placer Order Number",
        ("OBR", 3) => "Filler Order Number",
        ("OBR", 4) => "Universal Service Identifier",
        ("OBR", 5) => "Priority",
        ("OBR", 6) => "Requested Date/Time",
        ("OBR", 7) => "Observation Date/Time",
        ("OBR", 8) => "Observation End Date/Time",
        ("OBR", 9) => "Collection Volume",
        ("OBR", 10) => "Collector Identifier",
        ("OBR", 11) => "Specimen Action Code",
        ("OBR", 16) => "Ordering Provider",
        ("OBR", 25) => "Result Status",

        ("OBX", 1) => "Set ID - OBX",
        ("OBX", 2) => "Value Type",
        ("OBX", 3) => "Observation Identifier",
        ("OBX", 4) => "Observation Sub-ID",
        ("OBX", 5) => "Observation Value",
        ("OBX", 6) => "Units",
        ("OBX", 7) => "References Range",
        ("OBX", 8) => "Abnormal Flags",
        ("OBX", 9) => "Probability",
        ("OBX", 10) => "Nature of Abnormal Test",
        ("OBX", 11) => "Observation Result Status",
        ("OBX", 14) => "Date/Time of the Observation",

        ("MSA", 1) => "Acknowledgment Code",
        ("MSA", 2) => "Message Control ID",
        ("MSA", 3) => "Text Message",
        ("MSA", 4) => "Expected Sequence Number",
        ("MSA", 6) => "Error Condition",

        ("NK1", 1) => "Set ID - NK1",
        ("NK1", 2) => "Name",
        ("NK1", 3) => "Relationship",
        ("NK1", 4) => "Address",
        ("NK1", 5) => "Phone Number",

        ("AL1", 1) => "Set ID - AL1",
        ("AL1", 2) => "Allergen Type Code",
        ("AL1", 3) => "Allergen Code/Mnemonic/Description",
        ("AL1", 4) => "Allergy Severity Code",
        ("AL1", 5) => "Allergy Reaction Code",
        ("AL1", 6) => "Identification Date",

        ("DG1", 1) => "Set ID - DG1",
        ("DG1", 2) => "Diagnosis Coding Method",
        ("DG1", 3) => "Diagnosis Code",
        ("DG1", 4) => "Diagnosis Description",
        ("DG1", 5) => "Diagnosis Date/Time",
        ("DG1", 6) => "Diagnosis Type",

        ("IN1", 1) => "Set ID - IN1",
        ("IN1", 2) => "Insurance Plan ID",
        ("IN1", 3) => "Insurance Company ID",
        ("IN1", 4) => "Insurance Company Name",
        ("IN1", 5) => "Insurance Company Address",

        ("ORC", 1) => "Order Control",
        ("ORC", 2) => "Placer Order Number",
        ("ORC", 3) => "Filler Order Number",
        ("ORC", 4) => "Placer Group Number",
        ("ORC", 5) => "Order Status",
        ("ORC", 9) => "Date/Time of Transaction",
        ("ORC", 12) => "Ordering Provider",

        _ => return None,
    };
    Some(d)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MSG: &str = "MSH|^~\\&|SENDINGAPP|SENDINGFAC|RECVAPP|RECVFAC|20240101120000||ADT^A01|MSG00001|P|2.5\rEVN|A01|20240101120000\rPID|1||123456^^^HOSPITAL^MR||DOE^JOHN^Q||19800101|M|||123 MAIN ST^^ANYTOWN^CA^90210";

    #[test]
    fn parses_msh_offset_and_encoding_chars() {
        let (segs, enc) = parse(MSG).unwrap();
        assert_eq!(enc.field, '|');
        assert_eq!(enc.component, '^');
        assert_eq!(enc.subcomponent, '&');
        assert_eq!(segs[0].name, "MSH");
        // MSH.1 = separator, MSH.2 = encoding chars, MSH.3 = sending app.
        assert_eq!(segs[0].fields[0], "|");
        assert_eq!(segs[0].fields[1], "^~\\&");
        assert_eq!(segs[0].fields[2], "SENDINGAPP");
        // MSH.9 = ADT^A01
        assert_eq!(segs[0].fields[8], "ADT^A01");
    }

    #[test]
    fn json_has_named_segments_fields_and_components() {
        let out = run(MSG, "json", true, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let msh = &v[0];
        assert_eq!(msh["segment"], "MSH");
        assert_eq!(msh["description"], "Message Header");
        // MSH.1 literal separator.
        assert_eq!(msh["fields"][0]["id"], "MSH.1");
        assert_eq!(msh["fields"][0]["value"], "|");
        assert_eq!(msh["fields"][0]["description"], "Field Separator");
        // MSH.9 message type split into components.
        assert_eq!(msh["fields"][8]["id"], "MSH.9");
        assert_eq!(msh["fields"][8]["description"], "Message Type");
        assert_eq!(msh["fields"][8]["components"][0], "ADT");
        assert_eq!(msh["fields"][8]["components"][1], "A01");
        // PID.5 patient name.
        let pid = &v[2];
        assert_eq!(pid["segment"], "PID");
        assert_eq!(pid["fields"][4]["id"], "PID.5");
        assert_eq!(pid["fields"][4]["description"], "Patient Name");
        assert_eq!(pid["fields"][4]["components"][0], "DOE");
        assert_eq!(pid["fields"][4]["components"][1], "JOHN");
    }

    #[test]
    fn csv_flattens_leaves_with_locations() {
        let out = run(MSG, "csv", true, true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Segment,Location,Value,Description");
        assert!(out.contains("MSH,MSH.1,|,Field Separator"));
        assert!(out.contains("MSH,MSH.9.1,ADT,Message Type"));
        assert!(out.contains("MSH,MSH.9.2,A01,Message Type"));
        assert!(out.contains("PID,PID.5.1,DOE,Patient Name"));
        assert!(out.contains("PID,PID.5.2,JOHN,Patient Name"));
        assert!(out.contains("PID,PID.3.4,HOSPITAL,Patient Identifier List"));
        // Empty leaves are skipped (PID.3 has empty components 2 and 3).
        assert!(!out.contains("PID.3.2,"));
    }

    #[test]
    fn repetitions_are_split() {
        let msg = "MSH|^~\\&|A\rPID|1||111~222^X||DOE^JOHN";
        let out = run(msg, "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let pid3 = &v[1]["fields"][2];
        assert_eq!(pid3["id"], "PID.3");
        assert_eq!(pid3["repetitions"][0], "111");
        assert_eq!(pid3["repetitions"][1]["components"][0], "222");
        assert_eq!(pid3["repetitions"][1]["components"][1], "X");
    }

    #[test]
    fn subcomponents_are_split() {
        let msg = "MSH|^~\\&|A\rPID|1||ID^^^AUTH&2.16.840.1&ISO";
        let out = run(msg, "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let comp4 = &v[1]["fields"][2]["components"][3];
        assert_eq!(comp4["subcomponents"][0], "AUTH");
        assert_eq!(comp4["subcomponents"][1], "2.16.840.1");
        assert_eq!(comp4["subcomponents"][2], "ISO");
    }

    #[test]
    fn escape_sequences_decode_when_enabled() {
        let msg = "MSH|^~\\&|A\rNTE|1||Line one\\.br\\Line two \\T\\ more \\F\\ pipe";
        let decoded = run(msg, "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(v[1]["fields"][2]["value"], "Line one\nLine two & more | pipe");
        // With unescape off, the raw escape text is preserved.
        let raw = run(msg, "json", false, false).unwrap();
        let v2: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v2[1]["fields"][2]["value"], "Line one\\.br\\Line two \\T\\ more \\F\\ pipe");
    }

    #[test]
    fn custom_delimiters_from_msh_are_honored() {
        // Field sep '#', component '@'.
        let msg = "MSH#@~\\&#A#B#C@D";
        let (segs, enc) = parse(msg).unwrap();
        assert_eq!(enc.field, '#');
        assert_eq!(enc.component, '@');
        assert_eq!(segs[0].fields[0], "#");
        // MSH.5 = "C@D" splits on the component char '@'.
        let out = run(msg, "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["fields"][4]["id"], "MSH.5");
        assert_eq!(v[0]["fields"][4]["components"][0], "C");
        assert_eq!(v[0]["fields"][4]["components"][1], "D");
    }

    #[test]
    fn descriptions_can_be_omitted() {
        let out = run(MSG, "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v[0].get("description").is_none());
        assert!(v[0]["fields"][0].get("description").is_none());
    }

    #[test]
    fn accepts_lf_line_endings() {
        let msg = "MSH|^~\\&|A\nEVN|A01";
        let (segs, _) = parse(msg).unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[1].name, "EVN");
    }

    #[test]
    fn csv_quotes_values_with_commas() {
        let msg = "MSH|^~\\&|A\rNTE|1||Hello, world";
        let out = run(msg, "csv", false, true).unwrap();
        assert!(out.contains("\"Hello, world\""));
    }

    #[test]
    fn empty_input_errors() {
        let err = run("   \n  ", "json", true, true).unwrap_err();
        assert!(err.contains("no HL7 segments found"));
    }

    #[test]
    fn line_starting_with_separator_errors() {
        let msg = "MSH|^~\\&|A\r|BADSEGMENT";
        let err = run(msg, "json", true, true).unwrap_err();
        assert!(err.contains("no segment name"));
    }

    #[test]
    fn unknown_output_errors() {
        let err = run("MSH|^~\\&|A", "xml", true, true).unwrap_err();
        assert!(err.contains("unknown output"));
    }
}
