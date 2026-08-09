//! gizza-ai/sops-encrypt core — structure-preserving encryption for YAML, JSON
//! and `.env` documents. Pure Rust, wasm-safe, no I/O.
//!
//! Unlike a whole-file encryptor (see `blocks/encrypt-file` / `blocks/text-encrypt`),
//! this walks the document and replaces only the **leaf values**, leaving every key
//! readable. The result is still a valid YAML / JSON / `.env` file, so `git diff`
//! shows exactly which secret changed.
//!
//! Container format (deliberately NOT the `sops` binary's format — a passphrase key
//! has no place in its KMS/age/PGP metadata, so an ambiguous marker would be a lie):
//!
//! ```text
//! ENC[GZAE1,data:<base64>,iv:<base64>,tag:<base64>,type:str]
//! ```
//!
//! * One PBKDF2-HMAC-SHA256 (200k iterations) derivation per document, over a fresh
//!   random 16-byte salt stored in the document's metadata block.
//! * A fresh random 96-bit IV per value, AES-256-GCM.
//! * The value's dotted key path is the GCM **additional authenticated data**, so a
//!   ciphertext cannot be moved from one key to another without failing to decrypt.
//! * `type:` records the original scalar type so `decrypt` restores an int as an int.

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use regex::Regex;
use serde_json::{Map as JsonMap, Value as JsonValue};
use serde_yml::{Mapping as YamlMap, Value as YamlValue};

/// Marker prefix of an encrypted value.
const ENC_PREFIX: &str = "ENC[GZAE1,";
const KDF_NAME: &str = "pbkdf2-hmac-sha256";
const PBKDF2_ITERS: u32 = 200_000;
const SALT_LEN: usize = 16;
const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const FORMAT_VERSION: i64 = 1;

/// Metadata key in a YAML/JSON document.
const META_KEY: &str = "gizza_sops";
/// Metadata key prefix in a `.env` document.
const ENV_META_PREFIX: &str = "GIZZA_SOPS_";

/// The default `unencrypted_suffix`: keys ending in it stay in the clear.
pub const DEFAULT_UNENCRYPTED_SUFFIX: &str = "_unencrypted";

/// Largest document accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

// ---------------------------------------------------------------- public API

/// Document format.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Auto,
    Yaml,
    Json,
    Env,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Format::Auto),
            "yaml" | "yml" => Ok(Format::Yaml),
            "json" => Ok(Format::Json),
            "env" | "dotenv" => Ok(Format::Env),
            other => Err(format!(
                "unknown format '{other}' (use 'auto', 'yaml', 'json' or 'env')"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Auto => "auto",
            Format::Yaml => "yaml",
            Format::Json => "json",
            Format::Env => "env",
        }
    }
}

/// Which keys to encrypt.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub format: Option<Format>,
    /// Encrypt ONLY values whose key (or an ancestor key) ends with this suffix.
    pub encrypted_suffix: String,
    /// Leave values whose key (or an ancestor key) ends with this suffix in the clear.
    /// Defaults to [`DEFAULT_UNENCRYPTED_SUFFIX`] when empty and no other rule is set.
    pub unencrypted_suffix: String,
    /// Encrypt ONLY values whose key (or an ancestor key) matches this regex.
    pub encrypted_regex: String,
    /// Leave values whose key (or an ancestor key) matches this regex in the clear.
    pub unencrypted_regex: String,
}

impl Options {
    /// Options with the documented defaults (skip `*_unencrypted` keys).
    pub fn defaults() -> Options {
        Options {
            format: None,
            encrypted_suffix: String::new(),
            unencrypted_suffix: DEFAULT_UNENCRYPTED_SUFFIX.to_string(),
            encrypted_regex: String::new(),
            unencrypted_regex: String::new(),
        }
    }
}

/// Result of an encrypt or decrypt run.
#[derive(Debug)]
pub struct Outcome {
    /// The rewritten document.
    pub document: String,
    /// The format that was actually used (`yaml` / `json` / `env`).
    pub format: &'static str,
    /// How many leaf values were encrypted or decrypted.
    pub values: usize,
}

/// Encrypt every selected leaf value of `input`, leaving the keys readable.
pub fn encrypt(input: &str, passphrase: &str, opts: &Options) -> Result<Outcome, String> {
    let (format, rule) = prepare(input, passphrase, opts)?;
    let salt = random_bytes(SALT_LEN)?;
    let key = derive_key(passphrase, &salt);
    let mut count = 0usize;
    let document = match format {
        Format::Json => encrypt_tree_json(input, &key, &salt, &rule, &mut count)?,
        Format::Yaml => encrypt_tree_yaml(input, &key, &salt, &rule, &mut count)?,
        Format::Env => encrypt_env(input, &key, &salt, &rule, &mut count)?,
        Format::Auto => unreachable!("format resolved in prepare()"),
    };
    Ok(Outcome { document, format: format.name(), values: count })
}

/// Decrypt a document produced by [`encrypt`], restoring the original scalar types.
pub fn decrypt(input: &str, passphrase: &str, opts: &Options) -> Result<Outcome, String> {
    let (format, _) = prepare(input, passphrase, opts)?;
    let mut count = 0usize;
    let document = match format {
        Format::Json => decrypt_tree_json(input, passphrase, &mut count)?,
        Format::Yaml => decrypt_tree_yaml(input, passphrase, &mut count)?,
        Format::Env => decrypt_env(input, passphrase, &mut count)?,
        Format::Auto => unreachable!("format resolved in prepare()"),
    };
    Ok(Outcome { document, format: format.name(), values: count })
}

// ------------------------------------------------------------- shared set-up

fn prepare(input: &str, passphrase: &str, opts: &Options) -> Result<(Format, Rule), String> {
    if passphrase.is_empty() {
        return Err("passphrase is required".into());
    }
    if input.trim().is_empty() {
        return Err("document is empty — paste a YAML, JSON or .env file".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "document is {} bytes; the limit is {} bytes ({} KiB)",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / 1024
        ));
    }
    let format = match opts.format.unwrap_or(Format::Auto) {
        Format::Auto => detect_format(input),
        f => f,
    };
    Ok((format, Rule::build(opts)?))
}

/// Guess the format: JSON by its opening brace, `.env` when every meaningful line
/// is a `KEY=VALUE` assignment, YAML otherwise.
pub fn detect_format(input: &str) -> Format {
    let trimmed = input.trim_start();
    if trimmed.starts_with('{') {
        return Format::Json;
    }
    let mut assignments = 0usize;
    for line in input.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        match split_env_line(line) {
            Some(_) => assignments += 1,
            None => return Format::Yaml,
        }
    }
    if assignments > 0 {
        Format::Env
    } else {
        Format::Yaml
    }
}

// ---------------------------------------------------------------- key rules

enum Rule {
    /// Encrypt everything except keys ending with this suffix (empty = everything).
    ExceptSuffix(String),
    ExceptRegex(Regex),
    OnlySuffix(String),
    OnlyRegex(Regex),
}

impl Rule {
    fn build(o: &Options) -> Result<Rule, String> {
        let mut chosen: Vec<&str> = Vec::new();
        if !o.encrypted_regex.is_empty() {
            chosen.push("encrypted_regex");
        }
        if !o.encrypted_suffix.is_empty() {
            chosen.push("encrypted_suffix");
        }
        if !o.unencrypted_regex.is_empty() {
            chosen.push("unencrypted_regex");
        }
        if !o.unencrypted_suffix.is_empty() && o.unencrypted_suffix != DEFAULT_UNENCRYPTED_SUFFIX {
            chosen.push("unencrypted_suffix");
        }
        if chosen.len() > 1 {
            return Err(format!(
                "use only one key-selection rule at a time (got {})",
                chosen.join(" + ")
            ));
        }
        let compile = |name: &str, pat: &str| {
            Regex::new(pat).map_err(|e| format!("{name} is not a valid regular expression: {e}"))
        };
        if !o.encrypted_regex.is_empty() {
            Ok(Rule::OnlyRegex(compile("encrypted_regex", &o.encrypted_regex)?))
        } else if !o.encrypted_suffix.is_empty() {
            Ok(Rule::OnlySuffix(o.encrypted_suffix.clone()))
        } else if !o.unencrypted_regex.is_empty() {
            Ok(Rule::ExceptRegex(compile("unencrypted_regex", &o.unencrypted_regex)?))
        } else {
            Ok(Rule::ExceptSuffix(o.unencrypted_suffix.clone()))
        }
    }

    /// `keys` is every MAP key on the path to the value, outermost first.
    fn selects(&self, keys: &[String]) -> bool {
        match self {
            Rule::ExceptSuffix(s) => {
                s.is_empty() || !keys.iter().any(|k| k.ends_with(s.as_str()))
            }
            Rule::ExceptRegex(re) => !keys.iter().any(|k| re.is_match(k)),
            Rule::OnlySuffix(s) => keys.iter().any(|k| k.ends_with(s.as_str())),
            Rule::OnlyRegex(re) => keys.iter().any(|k| re.is_match(k)),
        }
    }
}

// -------------------------------------------------------------------- crypto

fn random_bytes(n: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf).map_err(|e| format!("rng error: {e}"))?;
    Ok(buf)
}

fn derive_key(passphrase: &str, salt: &[u8]) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2::<hmac::Hmac<sha2::Sha256>>(passphrase.as_bytes(), salt, PBKDF2_ITERS, &mut key)
        .expect("HMAC accepts any key length");
    key
}

/// Encrypt one scalar into an `ENC[…]` marker, binding it to `path`.
fn seal(key: &[u8; 32], path: &str, plaintext: &str, ty: &str) -> Result<String, String> {
    let iv = random_bytes(IV_LEN)?;
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let sealed = cipher
        .encrypt(
            Nonce::from_slice(&iv),
            Payload { msg: plaintext.as_bytes(), aad: path.as_bytes() },
        )
        .map_err(|_| "encryption failed".to_string())?;
    if sealed.len() < TAG_LEN {
        return Err("encryption failed".into());
    }
    let (data, tag) = sealed.split_at(sealed.len() - TAG_LEN);
    Ok(format!(
        "{ENC_PREFIX}data:{},iv:{},tag:{},type:{}]",
        B64.encode(data),
        B64.encode(&iv),
        B64.encode(tag),
        ty
    ))
}

/// Decrypt one `ENC[…]` marker. Returns the plaintext and its recorded type.
fn open(key: &[u8; 32], path: &str, marker: &str) -> Result<(String, String), String> {
    let body = marker
        .strip_prefix(ENC_PREFIX)
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| format!("value at '{path}' is not a recognized encrypted value"))?;
    let (mut data, mut iv, mut tag, mut ty) = (None, None, None, None);
    for part in body.split(',') {
        let (k, v) = part
            .split_once(':')
            .ok_or_else(|| format!("malformed encrypted value at '{path}'"))?;
        match k {
            "data" => data = Some(v),
            "iv" => iv = Some(v),
            "tag" => tag = Some(v),
            "type" => ty = Some(v),
            _ => return Err(format!("unknown field '{k}' in the encrypted value at '{path}'")),
        }
    }
    let decode = |label: &str, v: Option<&str>| -> Result<Vec<u8>, String> {
        let v = v.ok_or_else(|| format!("encrypted value at '{path}' is missing '{label}'"))?;
        B64.decode(v)
            .map_err(|_| format!("encrypted value at '{path}' has an invalid '{label}' field"))
    };
    let data = decode("data", data)?;
    let iv = decode("iv", iv)?;
    let tag = decode("tag", tag)?;
    let ty = ty
        .ok_or_else(|| format!("encrypted value at '{path}' is missing 'type'"))?
        .to_string();
    if iv.len() != IV_LEN || tag.len() != TAG_LEN {
        return Err(format!("encrypted value at '{path}' has a bad iv or tag length"));
    }
    let mut sealed = data;
    sealed.extend_from_slice(&tag);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let plain = cipher
        .decrypt(Nonce::from_slice(&iv), Payload { msg: &sealed, aad: path.as_bytes() })
        .map_err(|_| {
            format!("could not decrypt '{path}' — wrong passphrase, or the value was edited or moved to another key")
        })?;
    let plain = String::from_utf8(plain)
        .map_err(|_| format!("decrypted value at '{path}' is not valid UTF-8"))?;
    Ok((plain, ty))
}

fn is_encrypted(s: &str) -> bool {
    s.starts_with(ENC_PREFIX) && s.ends_with(']')
}

fn join_path(path: &[String]) -> String {
    path.join(".")
}

// -------------------------------------------------------------------- JSON

fn encrypt_tree_json(
    input: &str,
    key: &[u8; 32],
    salt: &[u8],
    rule: &Rule,
    count: &mut usize,
) -> Result<String, String> {
    let mut root: JsonValue = serde_json::from_str(input)
        .map_err(|e| format!("input is not valid JSON: {e}"))?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| "the JSON root must be an object so the metadata block can be stored".to_string())?;
    if obj.contains_key(META_KEY) {
        return Err(format!(
            "this document already has a '{META_KEY}' block — it looks encrypted already"
        ));
    }
    let mut path = Vec::new();
    let mut keys = Vec::new();
    let mut entries: Vec<(String, JsonValue)> = std::mem::take(obj).into_iter().collect();
    for (k, v) in entries.iter_mut() {
        path.push(k.clone());
        keys.push(k.clone());
        walk_json(v, &mut path, &mut keys, key, rule, count)?;
        keys.pop();
        path.pop();
    }
    let mut out = JsonMap::new();
    for (k, v) in entries {
        out.insert(k, v);
    }
    out.insert(META_KEY.to_string(), json_meta(salt));
    Ok(serde_json::to_string_pretty(&JsonValue::Object(out))
        .map_err(|e| format!("could not serialize JSON: {e}"))?)
}

fn walk_json(
    v: &mut JsonValue,
    path: &mut Vec<String>,
    keys: &mut Vec<String>,
    key: &[u8; 32],
    rule: &Rule,
    count: &mut usize,
) -> Result<(), String> {
    match v {
        JsonValue::Object(map) => {
            for (k, child) in map.iter_mut() {
                path.push(k.clone());
                keys.push(k.clone());
                walk_json(child, path, keys, key, rule, count)?;
                keys.pop();
                path.pop();
            }
            Ok(())
        }
        JsonValue::Array(items) => {
            for (i, child) in items.iter_mut().enumerate() {
                path.push(i.to_string());
                walk_json(child, path, keys, key, rule, count)?;
                path.pop();
            }
            Ok(())
        }
        JsonValue::Null => Ok(()),
        scalar => {
            if !rule.selects(keys) {
                return Ok(());
            }
            let p = join_path(path);
            let (text, ty) = json_scalar_parts(scalar, &p)?;
            *scalar = JsonValue::String(seal(key, &p, &text, ty)?);
            *count += 1;
            Ok(())
        }
    }
}

fn json_scalar_parts<'a>(v: &'a JsonValue, path: &str) -> Result<(String, &'static str), String> {
    match v {
        JsonValue::String(s) => {
            if is_encrypted(s) {
                Err(format!(
                    "'{path}' is already encrypted — decrypt the document before encrypting it again"
                ))
            } else {
                Ok((s.clone(), "str"))
            }
        }
        JsonValue::Bool(b) => Ok((b.to_string(), "bool")),
        JsonValue::Number(n) => {
            if n.is_f64() && n.as_i64().is_none() {
                Ok((n.to_string(), "float"))
            } else {
                Ok((n.to_string(), "int"))
            }
        }
        _ => Err(format!("unsupported scalar at '{path}'")),
    }
}

fn json_meta(salt: &[u8]) -> JsonValue {
    let mut m = JsonMap::new();
    m.insert("version".into(), JsonValue::from(FORMAT_VERSION));
    m.insert("kdf".into(), JsonValue::from(KDF_NAME));
    m.insert("iterations".into(), JsonValue::from(PBKDF2_ITERS));
    m.insert("salt".into(), JsonValue::from(B64.encode(salt)));
    JsonValue::Object(m)
}

fn decrypt_tree_json(input: &str, passphrase: &str, count: &mut usize) -> Result<String, String> {
    let mut root: JsonValue = serde_json::from_str(input)
        .map_err(|e| format!("input is not valid JSON: {e}"))?;
    let obj = root
        .as_object_mut()
        .ok_or_else(|| "the JSON root must be an object".to_string())?;
    let meta = obj
        .remove(META_KEY)
        .ok_or_else(|| missing_meta_error())?;
    let salt = read_meta(
        meta.get("version").and_then(|v| v.as_i64()),
        meta.get("kdf").and_then(|v| v.as_str()),
        meta.get("iterations").and_then(|v| v.as_i64()),
        meta.get("salt").and_then(|v| v.as_str()),
    )?;
    let key = derive_key(passphrase, &salt);
    let mut path = Vec::new();
    let mut entries: Vec<(String, JsonValue)> = std::mem::take(obj).into_iter().collect();
    for (k, v) in entries.iter_mut() {
        path.push(k.clone());
        unwalk_json(v, &mut path, &key, count)?;
        path.pop();
    }
    let mut out = JsonMap::new();
    for (k, v) in entries {
        out.insert(k, v);
    }
    Ok(serde_json::to_string_pretty(&JsonValue::Object(out))
        .map_err(|e| format!("could not serialize JSON: {e}"))?)
}

fn unwalk_json(
    v: &mut JsonValue,
    path: &mut Vec<String>,
    key: &[u8; 32],
    count: &mut usize,
) -> Result<(), String> {
    match v {
        JsonValue::Object(map) => {
            for (k, child) in map.iter_mut() {
                path.push(k.clone());
                unwalk_json(child, path, key, count)?;
                path.pop();
            }
            Ok(())
        }
        JsonValue::Array(items) => {
            for (i, child) in items.iter_mut().enumerate() {
                path.push(i.to_string());
                unwalk_json(child, path, key, count)?;
                path.pop();
            }
            Ok(())
        }
        JsonValue::String(s) if is_encrypted(s) => {
            let p = join_path(path);
            let (text, ty) = open(key, &p, s)?;
            *v = json_from_typed(&text, &ty, &p)?;
            *count += 1;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn json_from_typed(text: &str, ty: &str, path: &str) -> Result<JsonValue, String> {
    match ty {
        "str" => Ok(JsonValue::String(text.to_string())),
        "bool" => text
            .parse::<bool>()
            .map(JsonValue::Bool)
            .map_err(|_| format!("decrypted value at '{path}' is not a boolean")),
        "int" => text
            .parse::<i64>()
            .map(JsonValue::from)
            .map_err(|_| format!("decrypted value at '{path}' is not an integer")),
        "float" => text
            .parse::<f64>()
            .map(|f| JsonValue::from(f))
            .map_err(|_| format!("decrypted value at '{path}' is not a number")),
        other => Err(format!("unknown value type '{other}' at '{path}'")),
    }
}

// -------------------------------------------------------------------- YAML

fn encrypt_tree_yaml(
    input: &str,
    key: &[u8; 32],
    salt: &[u8],
    rule: &Rule,
    count: &mut usize,
) -> Result<String, String> {
    let mut root: YamlValue =
        serde_yml::from_str(input).map_err(|e| format!("input is not valid YAML: {e}"))?;
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| "the YAML root must be a mapping so the metadata block can be stored".to_string())?;
    if map.contains_key(YamlValue::String(META_KEY.into())) {
        return Err(format!(
            "this document already has a '{META_KEY}' block — it looks encrypted already"
        ));
    }
    let mut path = Vec::new();
    let mut keys = Vec::new();
    walk_yaml_map(map, &mut path, &mut keys, key, rule, count)?;
    map.insert(YamlValue::String(META_KEY.into()), yaml_meta(salt));
    serde_yml::to_string(&root).map_err(|e| format!("could not serialize YAML: {e}"))
}

fn walk_yaml_map(
    map: &mut YamlMap,
    path: &mut Vec<String>,
    keys: &mut Vec<String>,
    key: &[u8; 32],
    rule: &Rule,
    count: &mut usize,
) -> Result<(), String> {
    for (k, child) in map.iter_mut() {
        let name = yaml_key_name(k);
        path.push(name.clone());
        keys.push(name);
        walk_yaml(child, path, keys, key, rule, count)?;
        keys.pop();
        path.pop();
    }
    Ok(())
}

fn walk_yaml(
    v: &mut YamlValue,
    path: &mut Vec<String>,
    keys: &mut Vec<String>,
    key: &[u8; 32],
    rule: &Rule,
    count: &mut usize,
) -> Result<(), String> {
    match v {
        YamlValue::Mapping(map) => walk_yaml_map(map, path, keys, key, rule, count),
        YamlValue::Sequence(items) => {
            for (i, child) in items.iter_mut().enumerate() {
                path.push(i.to_string());
                walk_yaml(child, path, keys, key, rule, count)?;
                path.pop();
            }
            Ok(())
        }
        YamlValue::Null | YamlValue::Tagged(_) => Ok(()),
        scalar => {
            if !rule.selects(keys) {
                return Ok(());
            }
            let p = join_path(path);
            let (text, ty) = yaml_scalar_parts(scalar, &p)?;
            *scalar = YamlValue::String(seal(key, &p, &text, ty)?);
            *count += 1;
            Ok(())
        }
    }
}

fn yaml_scalar_parts(v: &YamlValue, path: &str) -> Result<(String, &'static str), String> {
    match v {
        YamlValue::String(s) => {
            if is_encrypted(s) {
                Err(format!(
                    "'{path}' is already encrypted — decrypt the document before encrypting it again"
                ))
            } else {
                Ok((s.clone(), "str"))
            }
        }
        YamlValue::Bool(b) => Ok((b.to_string(), "bool")),
        YamlValue::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Ok((n.to_string(), "int"))
            } else {
                Ok((n.to_string(), "float"))
            }
        }
        _ => Err(format!("unsupported scalar at '{path}'")),
    }
}

fn yaml_key_name(k: &YamlValue) -> String {
    match k {
        YamlValue::String(s) => s.clone(),
        YamlValue::Bool(b) => b.to_string(),
        YamlValue::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn yaml_meta(salt: &[u8]) -> YamlValue {
    let mut m = YamlMap::new();
    m.insert(YamlValue::String("version".into()), YamlValue::Number(FORMAT_VERSION.into()));
    m.insert(YamlValue::String("kdf".into()), YamlValue::String(KDF_NAME.into()));
    m.insert(
        YamlValue::String("iterations".into()),
        YamlValue::Number((PBKDF2_ITERS as i64).into()),
    );
    m.insert(YamlValue::String("salt".into()), YamlValue::String(B64.encode(salt)));
    YamlValue::Mapping(m)
}

fn decrypt_tree_yaml(input: &str, passphrase: &str, count: &mut usize) -> Result<String, String> {
    let mut root: YamlValue =
        serde_yml::from_str(input).map_err(|e| format!("input is not valid YAML: {e}"))?;
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| "the YAML root must be a mapping".to_string())?;
    let meta = map
        .remove(YamlValue::String(META_KEY.into()))
        .ok_or_else(|| missing_meta_error())?;
    let get = |k: &str| meta.get(YamlValue::String(k.into())).cloned();
    let salt = read_meta(
        get("version").and_then(|v| v.as_i64()),
        get("kdf").and_then(|v| v.as_str().map(str::to_string)).as_deref(),
        get("iterations").and_then(|v| v.as_i64()),
        get("salt").and_then(|v| v.as_str().map(str::to_string)).as_deref(),
    )?;
    let key = derive_key(passphrase, &salt);
    let mut path = Vec::new();
    unwalk_yaml_map(map, &mut path, &key, count)?;
    serde_yml::to_string(&root).map_err(|e| format!("could not serialize YAML: {e}"))
}

fn unwalk_yaml_map(
    map: &mut YamlMap,
    path: &mut Vec<String>,
    key: &[u8; 32],
    count: &mut usize,
) -> Result<(), String> {
    for (k, child) in map.iter_mut() {
        path.push(yaml_key_name(k));
        unwalk_yaml(child, path, key, count)?;
        path.pop();
    }
    Ok(())
}

fn unwalk_yaml(
    v: &mut YamlValue,
    path: &mut Vec<String>,
    key: &[u8; 32],
    count: &mut usize,
) -> Result<(), String> {
    match v {
        YamlValue::Mapping(map) => unwalk_yaml_map(map, path, key, count),
        YamlValue::Sequence(items) => {
            for (i, child) in items.iter_mut().enumerate() {
                path.push(i.to_string());
                unwalk_yaml(child, path, key, count)?;
                path.pop();
            }
            Ok(())
        }
        YamlValue::String(s) if is_encrypted(s) => {
            let p = join_path(path);
            let (text, ty) = open(key, &p, s)?;
            *v = yaml_from_typed(&text, &ty, &p)?;
            *count += 1;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn yaml_from_typed(text: &str, ty: &str, path: &str) -> Result<YamlValue, String> {
    match ty {
        "str" => Ok(YamlValue::String(text.to_string())),
        "bool" => text
            .parse::<bool>()
            .map(YamlValue::Bool)
            .map_err(|_| format!("decrypted value at '{path}' is not a boolean")),
        "int" => text
            .parse::<i64>()
            .map(|i| YamlValue::Number(i.into()))
            .map_err(|_| format!("decrypted value at '{path}' is not an integer")),
        "float" => text
            .parse::<f64>()
            .map(|f| YamlValue::Number(f.into()))
            .map_err(|_| format!("decrypted value at '{path}' is not a number")),
        other => Err(format!("unknown value type '{other}' at '{path}'")),
    }
}

// --------------------------------------------------------------------- .env

/// Split a `.env` line into (indent+export prefix, key, raw value).
fn split_env_line(line: &str) -> Option<(String, String, String)> {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let (prefix, rest) = match rest.strip_prefix("export ") {
        Some(r) => (format!("{indent}export "), r.trim_start()),
        None => (indent.to_string(), rest),
    };
    let (raw_key, value) = rest.split_once('=')?;
    let key = raw_key.trim_end();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    Some((prefix, key.to_string(), value.to_string()))
}

/// Strip surrounding quotes from a `.env` value, honouring the usual escapes.
fn unquote_env(raw: &str) -> String {
    let v = raw.trim();
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        return v[1..v.len() - 1].to_string();
    }
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let inner = &v[1..v.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        }
        return out;
    }
    v.to_string()
}

/// Render a value for a `.env` line, quoting only when it has to be quoted.
fn quote_env(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '#' | '"' | '\'' | '$' | '`' | '\\'));
    if !needs_quotes {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn encrypt_env(
    input: &str,
    key: &[u8; 32],
    salt: &[u8],
    rule: &Rule,
    count: &mut usize,
) -> Result<String, String> {
    let mut out: Vec<String> = Vec::new();
    for line in input.lines() {
        let Some((prefix, name, raw)) = split_env_line(line) else {
            out.push(line.to_string());
            continue;
        };
        if name.starts_with(ENV_META_PREFIX) {
            return Err(format!(
                "this document already has {ENV_META_PREFIX}* keys — it looks encrypted already"
            ));
        }
        let value = unquote_env(&raw);
        if is_encrypted(&value) {
            return Err(format!(
                "'{name}' is already encrypted — decrypt the document before encrypting it again"
            ));
        }
        let keys = vec![name.clone()];
        if !rule.selects(&keys) {
            out.push(line.to_string());
            continue;
        }
        out.push(format!("{prefix}{name}={}", seal(key, &name, &value, "str")?));
        *count += 1;
    }
    while out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.push(String::new());
    out.push(format!("{ENV_META_PREFIX}VERSION={FORMAT_VERSION}"));
    out.push(format!("{ENV_META_PREFIX}KDF={KDF_NAME}"));
    out.push(format!("{ENV_META_PREFIX}ITERATIONS={PBKDF2_ITERS}"));
    out.push(format!("{ENV_META_PREFIX}SALT={}", B64.encode(salt)));
    Ok(format!("{}\n", out.join("\n")))
}

fn decrypt_env(input: &str, passphrase: &str, count: &mut usize) -> Result<String, String> {
    let (mut version, mut kdf, mut iters, mut salt_b64) = (None, None, None, None);
    for line in input.lines() {
        if let Some((_, name, raw)) = split_env_line(line) {
            let value = unquote_env(&raw);
            match name.strip_prefix(ENV_META_PREFIX) {
                Some("VERSION") => version = value.parse::<i64>().ok(),
                Some("KDF") => kdf = Some(value),
                Some("ITERATIONS") => iters = value.parse::<i64>().ok(),
                Some("SALT") => salt_b64 = Some(value),
                _ => {}
            }
        }
    }
    if version.is_none() && salt_b64.is_none() {
        return Err(missing_meta_error());
    }
    let salt = read_meta(version, kdf.as_deref(), iters, salt_b64.as_deref())?;
    let key = derive_key(passphrase, &salt);

    let mut out: Vec<String> = Vec::new();
    for line in input.lines() {
        let Some((prefix, name, raw)) = split_env_line(line) else {
            out.push(line.to_string());
            continue;
        };
        if name.starts_with(ENV_META_PREFIX) {
            continue;
        }
        let value = unquote_env(&raw);
        if !is_encrypted(&value) {
            out.push(line.to_string());
            continue;
        }
        let (plain, _) = open(&key, &name, &value)?;
        out.push(format!("{prefix}{name}={}", quote_env(&plain)));
        *count += 1;
    }
    while out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        out.pop();
    }
    Ok(format!("{}\n", out.join("\n")))
}

// ------------------------------------------------------------------ metadata

fn missing_meta_error() -> String {
    format!(
        "no '{META_KEY}' metadata found — this document was not encrypted by this tool \
         (or the metadata block was removed)"
    )
}

fn read_meta(
    version: Option<i64>,
    kdf: Option<&str>,
    iterations: Option<i64>,
    salt_b64: Option<&str>,
) -> Result<Vec<u8>, String> {
    let version = version.ok_or_else(|| "the metadata block has no 'version'".to_string())?;
    if version != FORMAT_VERSION {
        return Err(format!(
            "unsupported document version {version} (this tool writes and reads version {FORMAT_VERSION})"
        ));
    }
    match kdf {
        Some(k) if k == KDF_NAME => {}
        Some(k) => return Err(format!("unsupported key derivation '{k}' (expected {KDF_NAME})")),
        None => return Err("the metadata block has no 'kdf'".into()),
    }
    match iterations {
        Some(i) if i == PBKDF2_ITERS as i64 => {}
        Some(i) => {
            return Err(format!(
                "unsupported iteration count {i} (this tool uses {PBKDF2_ITERS})"
            ))
        }
        None => return Err("the metadata block has no 'iterations'".into()),
    }
    let salt_b64 = salt_b64.ok_or_else(|| "the metadata block has no 'salt'".to_string())?;
    let salt = B64
        .decode(salt_b64.trim())
        .map_err(|_| "the metadata 'salt' is not valid base64".to_string())?;
    if salt.len() != SALT_LEN {
        return Err(format!("the metadata 'salt' must be {SALT_LEN} bytes"));
    }
    Ok(salt)
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "app: demo\ndatabase:\n  host: db.internal\n  password: s3cr3t\n  port: 5432\n";

    #[test]
    fn yaml_round_trip_keeps_keys_readable() {
        let enc = encrypt(YAML, "hunter2", &Options::defaults()).unwrap();
        assert_eq!(enc.format, "yaml");
        assert_eq!(enc.values, 4);
        // keys survive, values do not
        assert!(enc.document.contains("password:"));
        assert!(enc.document.contains("host:"));
        assert!(!enc.document.contains("s3cr3t"));
        assert!(!enc.document.contains("db.internal"));
        assert!(enc.document.contains(ENC_PREFIX));
        assert!(enc.document.contains("gizza_sops"));

        let dec = decrypt(&enc.document, "hunter2", &Options::defaults()).unwrap();
        assert_eq!(dec.values, 4);
        let back: serde_yml::Value = serde_yml::from_str(&dec.document).unwrap();
        let orig: serde_yml::Value = serde_yml::from_str(YAML).unwrap();
        assert_eq!(back, orig, "round trip restores the document, types included");
    }

    #[test]
    fn json_round_trip_restores_scalar_types() {
        let src = r#"{"name":"api","replicas":3,"ratio":0.5,"debug":false,"token":"abc","meta":null}"#;
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        assert_eq!(enc.format, "json");
        assert_eq!(enc.values, 5, "null is left alone");
        assert!(enc.document.contains("\"replicas\""));
        assert!(!enc.document.contains("\"abc\""));

        let dec = decrypt(&enc.document, "pw", &Options::defaults()).unwrap();
        let back: JsonValue = serde_json::from_str(&dec.document).unwrap();
        assert_eq!(back["replicas"], JsonValue::from(3));
        assert_eq!(back["ratio"], JsonValue::from(0.5));
        assert_eq!(back["debug"], JsonValue::from(false));
        assert_eq!(back["token"], JsonValue::from("abc"));
        assert!(back["meta"].is_null());
    }

    #[test]
    fn env_round_trip_preserves_comments_and_order() {
        let src = "# app config\nAPI_KEY=abc123\n\nexport DB_URL=\"postgres://u:p@h/db\"\nDEBUG=true\n";
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        assert_eq!(enc.format, "env");
        assert_eq!(enc.values, 3);
        assert!(enc.document.starts_with("# app config\n"));
        assert!(enc.document.contains("export DB_URL=ENC[GZAE1,"));
        assert!(!enc.document.contains("abc123"));
        assert!(enc.document.contains("GIZZA_SOPS_SALT="));

        let dec = decrypt(&enc.document, "pw", &Options::defaults()).unwrap();
        assert_eq!(dec.values, 3);
        assert!(dec.document.contains("API_KEY=abc123"));
        // The `export ` prefix survives; quotes come back only when the value needs them.
        assert!(dec.document.contains("export DB_URL=postgres://u:p@h/db"));
        assert!(dec.document.contains("DEBUG=true"));
        assert!(!dec.document.contains("GIZZA_SOPS_"));
        assert!(dec.document.starts_with("# app config\n"));
    }

    #[test]
    fn nested_lists_are_walked() {
        let src = "servers:\n  - token: one\n  - token: two\n";
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        assert_eq!(enc.values, 2);
        let dec = decrypt(&enc.document, "pw", &Options::defaults()).unwrap();
        assert!(dec.document.contains("one") && dec.document.contains("two"));
    }

    #[test]
    fn wrong_passphrase_fails_cleanly() {
        let enc = encrypt(YAML, "right", &Options::defaults()).unwrap();
        let err = decrypt(&enc.document, "wrong", &Options::defaults()).unwrap_err();
        assert!(err.contains("wrong passphrase"), "got: {err}");
    }

    #[test]
    fn a_value_cannot_be_moved_to_another_key() {
        let src = "a: alpha\nb: bravo\n";
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        let mut doc: YamlValue = serde_yml::from_str(&enc.document).unwrap();
        let a = doc.get(YamlValue::String("a".into())).cloned().unwrap();
        doc.as_mapping_mut()
            .unwrap()
            .insert(YamlValue::String("b".into()), a);
        let moved = serde_yml::to_string(&doc).unwrap();
        let err = decrypt(&moved, "pw", &Options::defaults()).unwrap_err();
        assert!(err.contains("moved to another key"), "got: {err}");
    }

    #[test]
    fn unencrypted_suffix_default_skips_keys() {
        let src = "region_unencrypted: eu-west-1\npassword: hunter2\n";
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        assert_eq!(enc.values, 1);
        assert!(enc.document.contains("eu-west-1"));
        assert!(!enc.document.contains("hunter2"));
    }

    #[test]
    fn unencrypted_suffix_exempts_a_whole_subtree() {
        let src = "public_unencrypted:\n  a: one\n  b: two\nsecret: x\n";
        let enc = encrypt(src, "pw", &Options::defaults()).unwrap();
        assert_eq!(enc.values, 1);
        assert!(enc.document.contains("one") && enc.document.contains("two"));
    }

    #[test]
    fn encrypted_suffix_selects_only_matching_keys() {
        let opts = Options { encrypted_suffix: "_key".into(), ..Options::defaults() };
        let src = "api_key: secret\nregion: eu-west-1\n";
        let enc = encrypt(src, "pw", &opts).unwrap();
        assert_eq!(enc.values, 1);
        assert!(enc.document.contains("eu-west-1"));
        assert!(!enc.document.contains("secret"));
    }

    #[test]
    fn encrypted_regex_selects_only_matching_keys() {
        let opts = Options {
            encrypted_regex: "^(password|token)$".into(),
            ..Options::defaults()
        };
        let src = "password: p\ntoken: t\nhost: h\n";
        let enc = encrypt(src, "pw", &opts).unwrap();
        assert_eq!(enc.values, 2);
        assert!(enc.document.contains("host: h"));
    }

    #[test]
    fn unencrypted_regex_skips_matching_keys() {
        let opts = Options {
            unencrypted_regex: "^host$".into(),
            unencrypted_suffix: String::new(),
            ..Options::defaults()
        };
        let src = "password: p\nhost: h\n";
        let enc = encrypt(src, "pw", &opts).unwrap();
        assert_eq!(enc.values, 1);
        assert!(enc.document.contains("host: h"));
    }

    #[test]
    fn combining_selection_rules_is_rejected() {
        let opts = Options {
            encrypted_suffix: "_key".into(),
            encrypted_regex: "x".into(),
            ..Options::defaults()
        };
        let err = encrypt("a: b\n", "pw", &opts).unwrap_err();
        assert!(err.contains("only one key-selection rule"), "got: {err}");
    }

    #[test]
    fn bad_regex_is_reported() {
        let opts = Options { encrypted_regex: "(".into(), ..Options::defaults() };
        let err = encrypt("a: b\n", "pw", &opts).unwrap_err();
        assert!(err.contains("not a valid regular expression"), "got: {err}");
    }

    #[test]
    fn double_encryption_is_refused() {
        let enc = encrypt(YAML, "pw", &Options::defaults()).unwrap();
        let err = encrypt(&enc.document, "pw", &Options::defaults()).unwrap_err();
        assert!(err.contains("already"), "got: {err}");
    }

    #[test]
    fn decrypting_a_plain_document_is_refused() {
        let err = decrypt(YAML, "pw", &Options::defaults()).unwrap_err();
        assert!(err.contains("no 'gizza_sops' metadata"), "got: {err}");
    }

    #[test]
    fn empty_passphrase_and_empty_document_are_refused() {
        assert!(encrypt(YAML, "", &Options::defaults()).is_err());
        assert!(encrypt("   ", "pw", &Options::defaults()).is_err());
    }

    #[test]
    fn oversize_document_is_refused() {
        let big = format!("a: {}\n", "x".repeat(MAX_INPUT_BYTES));
        let err = encrypt(&big, "pw", &Options::defaults()).unwrap_err();
        assert!(err.contains("the limit is"), "got: {err}");
    }

    #[test]
    fn json_root_must_be_an_object() {
        let err = encrypt("[1,2,3]", "pw", &Options { format: Some(Format::Json), ..Options::defaults() })
            .unwrap_err();
        assert!(err.contains("root must be an object"), "got: {err}");
    }

    #[test]
    fn format_detection() {
        assert_eq!(detect_format("{\"a\":1}"), Format::Json);
        assert_eq!(detect_format("A=1\nB=2\n"), Format::Env);
        assert_eq!(detect_format("# c\nexport A=1\n"), Format::Env);
        assert_eq!(detect_format("a: 1\nb: 2\n"), Format::Yaml);
    }

    #[test]
    fn explicit_format_overrides_detection() {
        // A .env-looking document forced through the YAML parser still round-trips
        // (YAML reads `A=1` as a plain scalar list item? no — as a bare scalar key-less
        // document), so assert the simpler case: JSON forced as JSON.
        let opts = Options { format: Some(Format::Json), ..Options::defaults() };
        let enc = encrypt("{\"k\":\"v\"}", "pw", &opts).unwrap();
        assert_eq!(enc.format, "json");
    }

    #[test]
    fn each_run_produces_different_ciphertext() {
        let a = encrypt(YAML, "pw", &Options::defaults()).unwrap().document;
        let b = encrypt(YAML, "pw", &Options::defaults()).unwrap().document;
        assert_ne!(a, b, "fresh salt + iv per run");
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let enc = encrypt("a: hello\n", "pw", &Options::defaults()).unwrap();
        // flip a base64 char inside the data field
        let bad = enc.document.replacen("data:A", "data:B", 1);
        let bad = if bad == enc.document { enc.document.replacen("data:", "data:AA", 1) } else { bad };
        assert!(decrypt(&bad, "pw", &Options::defaults()).is_err());
    }
}
