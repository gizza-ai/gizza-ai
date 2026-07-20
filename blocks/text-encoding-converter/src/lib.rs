//! gizza-ai/text-encoding-converter — detect the character encoding of a text
//! file's raw bytes and convert between UTF-8, UTF-16, Shift_JIS, EUC-JP, GBK,
//! Big5, Latin-1/Windows-1252 and the rest of the WHATWG charset set (the
//! iconv + chardet job). Pure Rust; chat + CLI (no standalone page — pure
//! blocks have no page file-upload runtime). For fixing mojibake in PASTED
//! text use charset-transcode; for binary file-format detection use
//! detect-file-type.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    build_media_envelope, AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_text_encoding_converter_core::{convert, detect, Conversion, Detection, Errors};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Input cap. Deliberately conservative: worst case the raw bytes, the decoded
/// UTF-8 string (≤3× for single-byte sources), the encoded output (≤2× the
/// string for UTF-16) and the base64 envelope (~2.7× the output) coexist in
/// the 64 MiB wasm sandbox.
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Envelope output cap (never reachable from a 4 MiB input; pure guard).
const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_mode")]
    mode: String,
    #[serde(default = "d_from")]
    from: String,
    #[serde(default = "d_to")]
    to: String,
    #[serde(default = "d_errors")]
    errors: String,
    #[serde(default)]
    bom: bool,
}

fn d_mode() -> String {
    "convert".into()
}
fn d_from() -> String {
    "auto".into()
}
fn d_to() -> String {
    "utf-8".into()
}
fn d_errors() -> String {
    "replace".into()
}

#[derive(Serialize)]
struct DetectResp {
    detected: String,
    method: String,
    bom: Option<String>,
    input_bytes: usize,
    valid_utf8: bool,
    ascii_only: bool,
    candidates: Vec<String>,
    preview: String,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv("mode", ["convert", "detect"])
                .default("convert")
                .describe("'convert' (default) re-encodes the file and returns it for download. 'detect' only reports the detected encoding (BOM, ASCII/UTF-8 validity, statistical guess, candidate charsets, text preview) without converting."),
        )
        .param(
            Param::string("from")
                .default("auto")
                .describe("Source charset of the file's bytes: 'auto' (default) sniffs a BOM (UTF-8/UTF-16/UTF-32, both endiannesses), then falls back to valid-UTF-8 detection and the chardetng statistical detector. Or any WHATWG label to override, e.g. 'shift_jis' (alias 'sjis'), 'euc-jp', 'iso-2022-jp', 'gbk', 'gb18030', 'big5', 'euc-kr', 'windows-1252', 'iso-8859-1' (alias 'latin1'), 'windows-1251', 'koi8-r', 'macintosh', 'utf-16le', 'utf-32le'."),
        )
        .param(
            Param::string("to")
                .default("utf-8")
                .describe("Target charset for the converted file (default 'utf-8'). Any WHATWG label with an encoder — e.g. 'utf-8', 'utf-16le', 'utf-16be', 'shift_jis', 'euc-jp', 'iso-2022-jp', 'gbk', 'gb18030', 'big5', 'euc-kr', 'windows-1252', 'iso-8859-15'. UTF-16 output always includes a BOM; UTF-32 output is not supported."),
        )
        .param(
            Param::enumv("errors", ["replace", "strict"])
                .default("replace")
                .describe("What to do with bytes invalid in 'from' or characters unencodable in 'to'. 'replace' (default) substitutes U+FFFD on decode / '?' on encode and reports the counts; 'strict' fails with the exact offset or character instead."),
        )
        .param(
            Param::boolean("bom")
                .default(false)
                .describe("Prepend a byte-order mark to UTF-8 output (default false — plain UTF-8 is the modern convention). UTF-16 targets always get a BOM regardless; combining bom=true with a legacy target is an error. Input BOMs are always stripped before converting."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Derive the output filename: insert the lowercased target-charset label
/// before the extension (`subs.srt` → `subs.utf-8.srt`); extensionless or
/// empty names fall back to `.txt`.
fn out_filename(in_name: &str, to_name: &str) -> String {
    let label = to_name.to_ascii_lowercase();
    let base = if in_name.trim().is_empty() {
        "file"
    } else {
        in_name.trim()
    };
    match base.rfind('.') {
        Some(i) if i > 0 && i + 1 < base.len() => {
            format!("{}.{label}.{}", &base[..i], &base[i + 1..])
        }
        _ => format!("{base}.{label}.txt"),
    }
}

/// Human phrase for how the source charset was determined.
fn method_phrase(method: &str) -> &'static str {
    match method {
        "bom" => " (from its byte-order mark)",
        "ascii" => " (pure 7-bit ASCII)",
        "valid-utf-8" => " (valid UTF-8)",
        "detector" => " (auto-detected)",
        _ => "",
    }
}

/// One-line summary of a conversion for the LLM/CLI.
fn convert_summary(c: &Conversion, in_name: &str, in_len: usize, out_name: &str) -> String {
    let mut repl = String::new();
    if c.replaced_decode > 0 {
        repl.push_str(&format!(
            "; {} invalid byte sequence(s) replaced with U+FFFD",
            c.replaced_decode
        ));
    }
    if c.replaced_encode > 0 {
        repl.push_str(&format!(
            "; {} unencodable character(s) replaced with '?'",
            c.replaced_encode
        ));
    }
    let bom_txt = match (c.bom_stripped, c.bom_written) {
        (Some(b), true) => format!("; {b} BOM stripped, output BOM written"),
        (Some(b), false) => format!("; {b} BOM stripped"),
        (None, true) => "; output BOM written".to_string(),
        (None, false) => String::new(),
    };
    format!(
        "converted {in_name} ({in_len} bytes, {}{}) → {out_name} ({} bytes, {}); {} characters{repl}{bom_txt}. Preview: {}",
        c.from_name,
        method_phrase(c.from_method),
        c.out.len(),
        c.to_name,
        c.chars,
        c.preview
    )
}

/// Explanatory note for the detect report, keyed on the detection method.
fn detect_note(d: &Detection) -> String {
    match d.method {
        "bom" => format!(
            "A {} byte-order mark pins the encoding exactly. Re-run with mode=convert to re-encode.",
            d.bom.unwrap_or("Unicode")
        ),
        "ascii" => "Pure 7-bit ASCII: the bytes are identical in UTF-8 and every ASCII-compatible charset, so no conversion is needed for UTF-8 targets. candidates lists charsets under which the bytes are also valid.".to_string(),
        "valid-utf-8" => "The bytes contain non-ASCII sequences that are valid UTF-8 — in practice that means UTF-8. Re-run with mode=convert to re-encode.".to_string(),
        _ => format!(
            "Statistical best guess (chardetng, the Firefox detector) over the first {} of the file; the detector gives no numeric confidence score. candidates lists multi-byte charsets under which the sampled bytes are entirely valid — if the guess looks wrong, re-run with an explicit from=<charset>.",
            "1 MiB"
        ),
    }
}

#[cfg(target_arch = "wasm32")]
struct TextEncodingConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-encoding-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect a text file's character encoding and convert it between UTF-8, UTF-16, Shift_JIS, GBK, Big5, Latin-1 and other charsets",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Detect the character encoding of a text file's RAW BYTES and convert it to another encoding (the iconv + chardet job): UTF-8 (with or without BOM), UTF-16LE/BE, UTF-32LE/BE input, Shift_JIS, EUC-JP, ISO-2022-JP, GBK, GB18030, Big5, EUC-KR, Windows-125x, ISO-8859-x/Latin-1, KOI8-R, Macintosh and every other WHATWG charset. mode=convert (default) returns the re-encoded file for download; mode=detect only reports: detected charset, how it was determined (BOM / pure ASCII / valid UTF-8 / chardetng statistical detector — no numeric confidence exists), any BOM, candidate multi-byte charsets whose byte sequences are all valid, and a decoded text preview. from defaults to 'auto' (BOM sniff, then UTF-8 validity, then statistical detection over the first 1 MiB); to defaults to 'utf-8'. errors=replace (default) substitutes U+FFFD/'?' and reports counts, errors=strict fails with the byte offset or character. bom=true adds a UTF-8 BOM (UTF-16 output always has one; input BOMs are always stripped). Input cap 4 MiB. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call). For garbled text PASTED into chat (mojibake repair) use charset-transcode instead; for identifying binary file formats use detect-file-type.",
        parameters = schema_json()
    ),
)]
impl TextEncodingConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("text-encoding-converter")?;
    let (bytes, _mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    if bytes.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the input file is empty (0 bytes) — nothing to detect or convert".into(),
        ));
    }
    match args.mode.as_str() {
        "detect" => {
            let d = detect(&bytes);
            let note = detect_note(&d);
            let resp = DetectResp {
                detected: d.encoding.clone(),
                method: d.method.to_string(),
                bom: d.bom.map(str::to_string),
                input_bytes: bytes.len(),
                valid_utf8: d.valid_utf8,
                ascii_only: d.ascii_only,
                candidates: d.candidates.clone(),
                preview: d.preview.clone(),
                note,
            };
            serde_json::to_vec(&resp).map_err(|e| {
                SkillError::Serialize(format!("serialize text-encoding-converter response: {e}"))
            })
        }
        "convert" => {
            let errors = Errors::parse(&args.errors).map_err(SkillError::InvalidArgs)?;
            let c = convert(&bytes, &args.from, &args.to, errors, args.bom)
                .map_err(SkillError::InvalidArgs)?;
            let name = out_filename(&in_name, &c.to_name);
            let for_llm = convert_summary(
                &c,
                if in_name.is_empty() { "file" } else { &in_name },
                bytes.len(),
                &name,
            );
            let mime = format!("text/plain;charset={}", c.to_name);
            build_media_envelope(&c.out, &mime, name, for_llm, MAX_OUTPUT_BYTES)
        }
        other => Err(SkillError::InvalidArgs(format!(
            "invalid mode {other:?}: expected \"convert\" or \"detect\""
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_applied() {
        let a: Args = serde_json::from_str(r#"{"url":"https://example.com/notes.txt"}"#).unwrap();
        assert_eq!(a.mode, "convert");
        assert_eq!(a.from, "auto");
        assert_eq!(a.to, "utf-8");
        assert_eq!(a.errors, "replace");
        assert!(!a.bom);
    }

    #[test]
    fn out_filename_inserts_label_before_extension() {
        assert_eq!(out_filename("subs.srt", "UTF-8"), "subs.utf-8.srt");
        assert_eq!(out_filename("readme", "Shift_JIS"), "readme.shift_jis.txt");
        assert_eq!(out_filename("", "UTF-16LE"), "file.utf-16le.txt");
        assert_eq!(out_filename(".hidden", "UTF-8"), ".hidden.utf-8.txt");
        assert_eq!(out_filename("a.b.c", "GBK"), "a.b.gbk.c");
    }

    #[test]
    fn convert_summary_reports_replacements_and_bom() {
        let c = Conversion {
            out: vec![0; 10],
            from_name: "Shift_JIS".into(),
            from_method: "detector",
            to_name: "UTF-8".into(),
            replaced_decode: 2,
            replaced_encode: 0,
            bom_written: false,
            bom_stripped: None,
            chars: 5,
            preview: "こんにちは".into(),
        };
        let s = convert_summary(&c, "notes.txt", 10, "notes.utf-8.txt");
        assert!(s.contains("Shift_JIS (auto-detected)"), "{s}");
        assert!(s.contains("2 invalid byte sequence(s)"), "{s}");
        assert!(s.contains("notes.utf-8.txt"), "{s}");
        assert!(s.contains("Preview: こんにちは"), "{s}");
    }

    #[test]
    fn detect_note_varies_by_method() {
        let mut d = Detection {
            encoding: "UTF-8".into(),
            method: "bom",
            bom: Some("UTF-8"),
            valid_utf8: true,
            ascii_only: false,
            candidates: vec!["UTF-8".into()],
            preview: "hi".into(),
        };
        assert!(detect_note(&d).contains("byte-order mark"));
        d.method = "detector";
        assert!(detect_note(&d).contains("no numeric confidence"));
        d.method = "ascii";
        assert!(detect_note(&d).contains("ASCII"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["convert", "detect"], "default": "convert", "description": "'convert' (default) re-encodes the file and returns it for download. 'detect' only reports the detected encoding (BOM, ASCII/UTF-8 validity, statistical guess, candidate charsets, text preview) without converting." },
                    "from": { "type": "string", "default": "auto", "description": "Source charset of the file's bytes: 'auto' (default) sniffs a BOM (UTF-8/UTF-16/UTF-32, both endiannesses), then falls back to valid-UTF-8 detection and the chardetng statistical detector. Or any WHATWG label to override, e.g. 'shift_jis' (alias 'sjis'), 'euc-jp', 'iso-2022-jp', 'gbk', 'gb18030', 'big5', 'euc-kr', 'windows-1252', 'iso-8859-1' (alias 'latin1'), 'windows-1251', 'koi8-r', 'macintosh', 'utf-16le', 'utf-32le'." },
                    "to": { "type": "string", "default": "utf-8", "description": "Target charset for the converted file (default 'utf-8'). Any WHATWG label with an encoder — e.g. 'utf-8', 'utf-16le', 'utf-16be', 'shift_jis', 'euc-jp', 'iso-2022-jp', 'gbk', 'gb18030', 'big5', 'euc-kr', 'windows-1252', 'iso-8859-15'. UTF-16 output always includes a BOM; UTF-32 output is not supported." },
                    "errors": { "type": "string", "enum": ["replace", "strict"], "default": "replace", "description": "What to do with bytes invalid in 'from' or characters unencodable in 'to'. 'replace' (default) substitutes U+FFFD on decode / '?' on encode and reports the counts; 'strict' fails with the exact offset or character instead." },
                    "bom": { "type": "boolean", "default": false, "description": "Prepend a byte-order mark to UTF-8 output (default false — plain UTF-8 is the modern convention). UTF-16 targets always get a BOM regardless; combining bom=true with a legacy target is an error. Input BOMs are always stripped before converting." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
