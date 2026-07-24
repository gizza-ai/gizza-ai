//! sd-png-metadata-reader core — extract Stable Diffusion generation parameters
//! from a PNG's text chunks (tEXt/iTXt/zTXt).
//!
//! Walks the PNG chunk stream at the BYTE level (no image decode), collects every
//! text chunk, and parses the AUTOMATIC1111 `parameters` format (positive prompt /
//! `Negative prompt:` / trailing `Key: value, …` settings line) into structured
//! fields — mirroring the receyuki/stable-diffusion-prompt-reader algorithm. Also
//! detects the generator (A1111 / ComfyUI / NovelAI / InvokeAI) heuristically and
//! returns every raw text chunk verbatim.
//!
//! Pure-Rust (`flate2` for zTXt / compressed-iTXt zlib inflate), wasm32-safe — no
//! image decode, no ffmpeg → runs on every backend including the chat Service Worker.

use std::collections::BTreeMap;
use std::io::Read;

use flate2::read::ZlibDecoder;
use serde::Serialize;

/// The 8-byte PNG file signature.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// Cap on a single zTXt/iTXt inflated payload — guards against a decompression bomb.
const MAX_DECOMPRESSED: u64 = 16 * 1024 * 1024;

/// A single decoded PNG text chunk, before any A1111 parsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawChunk {
    /// The chunk type: "tEXt", "zTXt", or "iTXt".
    pub chunk_type: String,
    /// The keyword (chunk key), e.g. "parameters", "prompt", "workflow", "Comment".
    pub keyword: String,
    /// The decoded (and, for zTXt/compressed-iTXt, decompressed) text value.
    pub text: String,
}

/// Structured Stable Diffusion metadata extracted from a PNG.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SdMeta {
    /// True when a Stable Diffusion generator was recognized (an A1111 `parameters`
    /// block was parsed, or a ComfyUI/NovelAI/InvokeAI signature was detected).
    pub has_sd_metadata: bool,
    /// Detected generator/tool when recognizable ("AUTOMATIC1111", "ComfyUI",
    /// "NovelAI", "InvokeAI"). Absent when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    /// The positive prompt (A1111 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The negative prompt (A1111 only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,
    /// Sampling steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<String>,
    /// Sampler name, e.g. "Euler a", "DPM++ 2M Karras".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampler: Option<String>,
    /// CFG (classifier-free guidance) scale.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cfg_scale: Option<String>,
    /// The seed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    /// The raw size string, e.g. "512x768".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Width parsed from `Size`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    /// Height parsed from `Size`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Model / checkpoint name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Model / checkpoint hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_hash: Option<String>,
    /// Every `Key: value` pair parsed from the A1111 settings line (sorted keys for
    /// stable output). Retains keys we don't lift into typed fields — Schedule type,
    /// VAE, Clip skip, Denoising strength, Hires*, Lora hashes, Version, ControlNet, …
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, String>,
    /// Every text chunk found in the PNG, raw (the "RAW view").
    pub raw_chunks: Vec<RawChunk>,
    /// Number of text chunks found.
    pub chunk_count: usize,
}

/// Read Stable Diffusion metadata from PNG bytes.
///
/// Errors only when the input is not a PNG. A PNG with no text chunks (or no SD
/// metadata) returns `has_sd_metadata: false` with `raw_chunks` still populated.
pub fn read(bytes: &[u8]) -> Result<SdMeta, String> {
    if bytes.len() < 8 || bytes[..8] != PNG_MAGIC {
        return Err("not a PNG file: this tool reads Stable Diffusion parameters from a PNG's \
                    text chunks (tEXt/iTXt/zTXt); the input did not start with the PNG signature"
            .to_string());
    }
    let chunks = extract_text_chunks(bytes);
    let mut meta = SdMeta {
        chunk_count: chunks.len(),
        ..Default::default()
    };
    if let Some(gen) = detect_generator(&chunks) {
        meta.generator = Some(gen);
    }
    if let Some(p) = chunks
        .iter()
        .find(|c| c.keyword.eq_ignore_ascii_case("parameters"))
    {
        parse_a1111(&p.text, &mut meta);
        meta.has_sd_metadata = true;
    } else if meta.generator.is_some() {
        // Non-A1111 metadata present (e.g. ComfyUI/NovelAI JSON) — the raw chunk is
        // the honest surface; we don't parse the version-specific node graph.
        meta.has_sd_metadata = true;
    }
    meta.raw_chunks = chunks;
    Ok(meta)
}

/// Walk the PNG chunk stream and decode every tEXt/zTXt/iTXt chunk. Malformed or
/// truncated input simply stops the walk (no error — partial reads are still useful).
fn extract_text_chunks(bytes: &[u8]) -> Vec<RawChunk> {
    let mut chunks = Vec::new();
    let mut pos = 8usize; // past the signature
    while pos + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = match data_start.checked_add(len) {
            Some(e) => e,
            None => break,
        };
        // Each chunk ends with a 4-byte CRC (which we don't verify).
        if data_end + 4 > bytes.len() {
            break;
        }
        let data = &bytes[data_start..data_end];
        match ctype {
            b"tEXt" => {
                if let Some(c) = parse_text(data) {
                    chunks.push(c);
                }
            }
            b"zTXt" => {
                if let Some(c) = parse_ztxt(data) {
                    chunks.push(c);
                }
            }
            b"iTXt" => {
                if let Some(c) = parse_itxt(data) {
                    chunks.push(c);
                }
            }
            b"IEND" => break,
            _ => {}
        }
        pos = data_end + 4;
    }
    chunks
}

/// Decode a byte slice as UTF-8, falling back to Latin-1 (PNG tEXt is spec'd Latin-1,
/// but A1111/Pillow write UTF-8 bytes; try UTF-8 first so non-ASCII prompts survive).
fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// tEXt: `keyword\0text` (uncompressed).
fn parse_text(data: &[u8]) -> Option<RawChunk> {
    let nul = data.iter().position(|&b| b == 0)?;
    Some(RawChunk {
        chunk_type: "tEXt".to_string(),
        keyword: decode_text(&data[..nul]),
        text: decode_text(&data[nul + 1..]),
    })
}

/// zTXt: `keyword\0[compression_method:1][zlib-compressed text]`.
fn parse_ztxt(data: &[u8]) -> Option<RawChunk> {
    let nul = data.iter().position(|&b| b == 0)?;
    let keyword = decode_text(&data[..nul]);
    let rest = &data[nul + 1..];
    if rest.is_empty() || rest[0] != 0 {
        return None; // only compression method 0 (zlib/deflate) is defined
    }
    let inflated = inflate(&rest[1..]).ok()?;
    Some(RawChunk {
        chunk_type: "zTXt".to_string(),
        keyword,
        text: decode_text(&inflated),
    })
}

/// iTXt: `keyword\0[comp_flag:1][comp_method:1]language\0translated_keyword\0text` (UTF-8).
fn parse_itxt(data: &[u8]) -> Option<RawChunk> {
    let n1 = data.iter().position(|&b| b == 0)?;
    let keyword = decode_text(&data[..n1]);
    let rest = &data[n1 + 1..];
    if rest.len() < 2 {
        return None;
    }
    let comp_flag = rest[0];
    let after = &rest[2..]; // skip comp_flag + comp_method
    let n2 = after.iter().position(|&b| b == 0)?; // end of language tag
    let after2 = &after[n2 + 1..];
    let n3 = after2.iter().position(|&b| b == 0)?; // end of translated keyword
    let text_bytes = &after2[n3 + 1..];
    let text = if comp_flag == 1 {
        let inflated = inflate(text_bytes).ok()?;
        String::from_utf8_lossy(&inflated).into_owned()
    } else {
        String::from_utf8_lossy(text_bytes).into_owned()
    };
    Some(RawChunk {
        chunk_type: "iTXt".to_string(),
        keyword,
        text,
    })
}

/// zlib inflate, capped at `MAX_DECOMPRESSED` bytes.
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    ZlibDecoder::new(data)
        .take(MAX_DECOMPRESSED)
        .read_to_end(&mut out)
        .map_err(|e| format!("zlib inflate failed: {e}"))?;
    Ok(out)
}

/// Heuristic generator detection from the set of text chunks.
fn detect_generator(chunks: &[RawChunk]) -> Option<String> {
    let has = |k: &str| chunks.iter().any(|c| c.keyword.eq_ignore_ascii_case(k));
    if has("parameters") {
        return Some("AUTOMATIC1111".to_string());
    }
    // ComfyUI writes "prompt" and/or "workflow" as JSON objects.
    if chunks.iter().any(|c| {
        (c.keyword.eq_ignore_ascii_case("workflow") || c.keyword.eq_ignore_ascii_case("prompt"))
            && c.text.trim_start().starts_with('{')
    }) {
        return Some("ComfyUI".to_string());
    }
    // NovelAI stamps Software = "NovelAI".
    if chunks
        .iter()
        .any(|c| c.keyword.eq_ignore_ascii_case("Software") && c.text.contains("NovelAI"))
    {
        return Some("NovelAI".to_string());
    }
    if has("invokeai_metadata") || has("invokeai_graph") || has("sd-metadata") || has("Dream") {
        return Some("InvokeAI".to_string());
    }
    None
}

/// Parse the A1111 `parameters` text into `meta` (prompt / negative / settings),
/// following the receyuki last-line algorithm: the final line is the comma-separated
/// settings blob when it parses to ≥3 `Key: value` pairs; otherwise it's part of the
/// prompt and there are no settings.
fn parse_a1111(raw: &str, meta: &mut SdMeta) {
    let text = raw.replace('\r', "");
    let text = text.trim_matches('\n');
    if text.is_empty() {
        return;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines[lines.len() - 1].trim();
    let settings = parse_settings_pairs(last);
    let has_settings = settings.len() >= 3;
    let body: &[&str] = if has_settings {
        &lines[..lines.len() - 1]
    } else {
        &lines[..]
    };

    let mut prompt = String::new();
    let mut negative = String::new();
    let mut in_neg = false;
    for line in body {
        if let Some(rest) = line.trim_start().strip_prefix("Negative prompt:") {
            in_neg = true;
            push_line(&mut negative, rest.trim_start());
        } else if in_neg {
            push_line(&mut negative, line);
        } else {
            push_line(&mut prompt, line);
        }
    }
    let prompt = prompt.trim().to_string();
    let negative = negative.trim().to_string();
    if !prompt.is_empty() {
        meta.prompt = Some(prompt);
    }
    if !negative.is_empty() {
        meta.negative_prompt = Some(negative);
    }

    for (k, v) in settings {
        match k.to_ascii_lowercase().as_str() {
            "steps" => meta.steps = Some(v.clone()),
            "sampler" => meta.sampler = Some(v.clone()),
            "cfg scale" => meta.cfg_scale = Some(v.clone()),
            "seed" => meta.seed = Some(v.clone()),
            "size" => {
                if let Some((w, h)) = parse_size(&v) {
                    meta.width = Some(w);
                    meta.height = Some(h);
                }
                meta.size = Some(v.clone());
            }
            "model" => meta.model = Some(v.clone()),
            "model hash" => meta.model_hash = Some(v.clone()),
            _ => {}
        }
        meta.params.insert(k, v);
    }
}

/// Append a line to an accumulating multi-line string.
fn push_line(acc: &mut String, line: &str) {
    if !acc.is_empty() {
        acc.push('\n');
    }
    acc.push_str(line);
}

/// Split the settings line into `Key: value` pairs, respecting double-quoted values
/// (so commas inside `Lora hashes: "a: 1, b: 2"` don't split the token) and
/// backslash-unescaping quoted values.
fn parse_settings_pairs(line: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for seg in split_top_commas(line) {
        let Some(idx) = seg.find(':') else { continue };
        let key = seg[..idx].trim();
        // A real key starts alphanumeric (rejects prompt fragments like ", :(").
        if key.is_empty() || !key.chars().next().unwrap().is_alphanumeric() {
            continue;
        }
        let mut val = seg[idx + 1..].trim().to_string();
        if val.len() >= 2 && val.starts_with('"') && val.ends_with('"') {
            val = unescape(&val[1..val.len() - 1]);
        }
        pairs.push((key.to_string(), val));
    }
    pairs
}

/// Split on commas that are not inside a double-quoted run.
fn split_top_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_q = !in_q;
                cur.push(c);
            }
            '\\' if in_q => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            ',' if !in_q => {
                let t = cur.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

/// Unescape `\"`, `\\`, `\n`, `\t`, `\r` inside a quoted value.
fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse an A1111 `Size` value like "512x768" (or "512×768") into (width, height).
fn parse_size(s: &str) -> Option<(u32, u32)> {
    let sep = s.find(['x', 'X', '×'])?;
    let w = s[..sep].trim().parse().ok()?;
    let h = s[sep + s[sep..].chars().next()?.len_utf8()..].trim().parse().ok()?;
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn chunk(ctype: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(ctype);
        v.extend_from_slice(data);
        v.extend_from_slice(&[0, 0, 0, 0]); // dummy CRC — the reader ignores it
        v
    }

    fn png_with(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut v = PNG_MAGIC.to_vec();
        for c in chunks {
            v.extend_from_slice(c);
        }
        v.extend_from_slice(&chunk(b"IEND", &[]));
        v
    }

    fn text_chunk(keyword: &str, text: &str) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(keyword.as_bytes());
        data.push(0);
        data.extend_from_slice(text.as_bytes());
        chunk(b"tEXt", &data)
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    const A1111: &str = "gorgeous scenery, snowy mountains\nNegative prompt: blurry, low quality\nSteps: 20, Sampler: Euler a, CFG scale: 7, Seed: 1234567890, Size: 512x768, Model hash: abc123def4, Model: v1-5-pruned-emaonly";

    #[test]
    fn parses_a1111_text_chunk() {
        let png = png_with(&[text_chunk("parameters", A1111)]);
        let m = read(&png).unwrap();
        assert!(m.has_sd_metadata);
        assert_eq!(m.generator.as_deref(), Some("AUTOMATIC1111"));
        assert_eq!(m.prompt.as_deref(), Some("gorgeous scenery, snowy mountains"));
        assert_eq!(m.negative_prompt.as_deref(), Some("blurry, low quality"));
        assert_eq!(m.steps.as_deref(), Some("20"));
        assert_eq!(m.sampler.as_deref(), Some("Euler a"));
        assert_eq!(m.cfg_scale.as_deref(), Some("7"));
        assert_eq!(m.seed.as_deref(), Some("1234567890"));
        assert_eq!(m.size.as_deref(), Some("512x768"));
        assert_eq!(m.width, Some(512));
        assert_eq!(m.height, Some(768));
        assert_eq!(m.model.as_deref(), Some("v1-5-pruned-emaonly"));
        assert_eq!(m.model_hash.as_deref(), Some("abc123def4"));
        assert_eq!(m.chunk_count, 1);
        assert_eq!(m.raw_chunks[0].chunk_type, "tEXt");
        // Unknown-but-present keys are retained in params.
        assert_eq!(m.params.get("CFG scale").map(String::as_str), Some("7"));
    }

    #[test]
    fn keeps_unknown_settings_and_quoted_commas() {
        let params = "cat\nNegative prompt: dog\nSteps: 30, Sampler: DPM++ 2M, Schedule type: Karras, CFG scale: 5, Clip skip: 2, Lora hashes: \"detailer: a1b2c3, style: d4e5f6\", Version: v1.10.1";
        let png = png_with(&[text_chunk("parameters", params)]);
        let m = read(&png).unwrap();
        assert_eq!(m.params.get("Schedule type").map(String::as_str), Some("Karras"));
        assert_eq!(m.params.get("Clip skip").map(String::as_str), Some("2"));
        assert_eq!(m.params.get("Version").map(String::as_str), Some("v1.10.1"));
        // The quoted value keeps its inner comma instead of being split.
        assert_eq!(
            m.params.get("Lora hashes").map(String::as_str),
            Some("detailer: a1b2c3, style: d4e5f6")
        );
        assert_eq!(m.sampler.as_deref(), Some("DPM++ 2M"));
    }

    #[test]
    fn parses_ztxt_compressed_chunk() {
        let mut data = Vec::new();
        data.extend_from_slice(b"parameters");
        data.push(0);
        data.push(0); // compression method 0
        data.extend_from_slice(&zlib(A1111.as_bytes()));
        let png = png_with(&[chunk(b"zTXt", &data)]);
        let m = read(&png).unwrap();
        assert!(m.has_sd_metadata);
        assert_eq!(m.raw_chunks[0].chunk_type, "zTXt");
        assert_eq!(m.seed.as_deref(), Some("1234567890"));
    }

    #[test]
    fn parses_itxt_uncompressed_chunk() {
        // keyword\0 comp_flag=0 comp_method=0 lang\0 transkw\0 text
        let mut data = Vec::new();
        data.extend_from_slice(b"parameters");
        data.push(0);
        data.push(0); // compression flag
        data.push(0); // compression method
        data.push(0); // empty language
        data.push(0); // empty translated keyword
        data.extend_from_slice(A1111.as_bytes());
        let png = png_with(&[chunk(b"iTXt", &data)]);
        let m = read(&png).unwrap();
        assert_eq!(m.raw_chunks[0].chunk_type, "iTXt");
        assert_eq!(m.width, Some(512));
    }

    #[test]
    fn no_negative_prompt_still_parses() {
        let params = "just a positive prompt\nSteps: 10, Sampler: DDIM, CFG scale: 8, Seed: 42";
        let png = png_with(&[text_chunk("parameters", params)]);
        let m = read(&png).unwrap();
        assert_eq!(m.prompt.as_deref(), Some("just a positive prompt"));
        assert!(m.negative_prompt.is_none());
        assert_eq!(m.seed.as_deref(), Some("42"));
    }

    #[test]
    fn detects_comfyui() {
        let png = png_with(&[text_chunk("workflow", "{\"nodes\": []}")]);
        let m = read(&png).unwrap();
        assert!(m.has_sd_metadata);
        assert_eq!(m.generator.as_deref(), Some("ComfyUI"));
        assert!(m.prompt.is_none()); // node graph is not parsed into typed fields
    }

    #[test]
    fn png_without_sd_metadata() {
        let png = png_with(&[text_chunk("Comment", "a plain caption")]);
        let m = read(&png).unwrap();
        assert!(!m.has_sd_metadata);
        assert!(m.generator.is_none());
        assert_eq!(m.chunk_count, 1);
        assert_eq!(m.raw_chunks[0].keyword, "Comment");
    }

    #[test]
    fn errors_on_non_png() {
        assert!(read(b"").is_err());
        assert!(read(b"\xff\xd8\xff\xe0not a png").is_err());
    }
}
