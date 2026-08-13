//! age-keygen core — generate or inspect age X25519 identities.
//!
//! Pure compute shared by the chat skill block, CLI descriptor, and browser page.
//! The only randomness is `age::x25519::Identity::generate()`, which uses the
//! platform CSPRNG (browser crypto in wasm, OS RNG on native).

use std::str::FromStr;

use age::secrecy::ExposeSecret;

pub const TEST_IDENTITY: &str =
    "AGE-SECRET-KEY-1GQ9778VQXMMJVE8SK7J6VT8UJ4HDQAJUVSFCWCM02D8GEWQ72PVQ2Y5J33";
pub const TEST_RECIPIENT: &str = "age1t7rxyev2z3rw82stdlrrepyc39nvn86l5078zqkf5uasdy86jp6svpy7pa";
pub const MAX_COMMENT_CHARS: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    RecipientOnly,
    IdentityOnly,
}

impl Format {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "recipient_only" | "recipient" | "public" => Ok(Self::RecipientOnly),
            "identity_only" | "identity" | "secret" => Ok(Self::IdentityOnly),
            other => Err(format!(
                "unknown format \"{other}\"; choose text, json, recipient_only, or identity_only"
            )),
        }
    }
}

fn clean_comment(comment: &str) -> Result<String, String> {
    let one_line = comment.trim().replace(['\r', '\n'], " ");
    if one_line.chars().count() > MAX_COMMENT_CHARS {
        return Err(format!(
            "comment is too long: maximum {MAX_COMMENT_CHARS} characters"
        ));
    }
    Ok(one_line)
}

fn identity_from_input(seed_or_identity: &str) -> Result<age::x25519::Identity, String> {
    let s = seed_or_identity.trim();
    if s.is_empty() {
        return Ok(age::x25519::Identity::generate());
    }
    if s.starts_with("AGE-SECRET-KEY-1") {
        return age::x25519::Identity::from_str(s)
            .map_err(|e| format!("identity is not a valid age X25519 secret key: {e}"));
    }
    Err("seed_or_identity must be empty for a fresh random key or a pasted AGE-SECRET-KEY-1... identity; raw hex seeds are deliberately not accepted because clamping and bech32 details are easy to get wrong outside the age implementation".into())
}

/// Generate a fresh age identity, or derive the recipient for a pasted identity.
///
/// * `format` — `text` | `json` | `recipient_only` | `identity_only`.
/// * `comment` — optional extra `# ...` line in text output.
/// * `include_created` — include an age-keygen-style created timestamp comment.
/// * `seed_or_identity` — empty for random generation; pasted identity for
///   deterministic tests / recipient derivation.
pub fn run(
    format: &str,
    comment: &str,
    include_created: bool,
    seed_or_identity: &str,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let comment = clean_comment(comment)?;
    let identity = identity_from_input(seed_or_identity)?;
    let identity_text = identity.to_string().expose_secret().to_string();
    let recipient = identity.to_public().to_string();

    match format {
        Format::RecipientOnly => Ok(recipient),
        Format::IdentityOnly => Ok(identity_text),
        Format::Json => {
            let mut obj = serde_json::Map::new();
            obj.insert("recipient".into(), serde_json::Value::String(recipient));
            obj.insert("identity".into(), serde_json::Value::String(identity_text));
            if !comment.is_empty() {
                obj.insert("comment".into(), serde_json::Value::String(comment));
            }
            Ok(serde_json::to_string_pretty(&serde_json::Value::Object(obj))
                .expect("static object serializes"))
        }
        Format::Text => {
            let mut out = String::new();
            if include_created {
                out.push_str("# created: ");
                out.push_str(&chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
                out.push('\n');
            }
            if !comment.is_empty() {
                out.push_str("# ");
                out.push_str(&comment);
                out.push('\n');
            }
            out.push_str("# public key: ");
            out.push_str(&recipient);
            out.push('\n');
            out.push_str(&identity_text);
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_identity_derives_known_recipient() {
        assert_eq!(
            run("recipient_only", "", false, TEST_IDENTITY).unwrap(),
            TEST_RECIPIENT
        );
    }

    #[test]
    fn identity_only_round_trips_the_pasted_secret() {
        assert_eq!(
            run("identity_only", "", false, TEST_IDENTITY).unwrap(),
            TEST_IDENTITY
        );
    }

    #[test]
    fn json_contains_identity_and_recipient() {
        let out = run("json", "laptop backup", false, TEST_IDENTITY).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["recipient"], TEST_RECIPIENT);
        assert_eq!(v["identity"], TEST_IDENTITY);
        assert_eq!(v["comment"], "laptop backup");
    }

    #[test]
    fn text_matches_age_keygen_shape_without_timestamp() {
        let out = run("text", "test key", false, TEST_IDENTITY).unwrap();
        assert_eq!(
            out,
            format!("# test key\n# public key: {TEST_RECIPIENT}\n{TEST_IDENTITY}")
        );
    }

    #[test]
    fn random_generation_looks_like_age_keys() {
        let out = run("json", "", true, "").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v["recipient"].as_str().unwrap().starts_with("age1"));
        assert!(v["identity"].as_str().unwrap().starts_with("AGE-SECRET-KEY-1"));
    }

    #[test]
    fn invalid_format_identity_and_comment_are_errors() {
        assert!(run("yaml", "", false, TEST_IDENTITY)
            .unwrap_err()
            .contains("unknown format"));
        assert!(run("text", "", false, "not-a-key")
            .unwrap_err()
            .contains("seed_or_identity"));
        assert!(run("text", &"x".repeat(MAX_COMMENT_CHARS + 1), false, "")
            .unwrap_err()
            .contains("comment is too long"));
    }
}
