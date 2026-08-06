//! age-encrypt core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Encrypts text to an ASCII-armored age file (`-----BEGIN AGE ENCRYPTED FILE-----`)
//! in one of two modes:
//!
//! * `passphrase` — an scrypt recipient stanza; anyone with the passphrase can decrypt.
//! * `recipients` — one or more X25519 public keys (`age1…`); only the matching
//!   identities can decrypt.
//!
//! The scrypt work factor is set EXPLICITLY (never left to the crate default). age's
//! default calibrates to ~1 second of CPU, which lands around `log_n = 18` — that is
//! `128 * 8 * 2^18` ≈ 268 MB of scratch memory, far past the 64 MiB wasm sandbox the
//! chat/CLI block runs in. [`MAX_WORK_FACTOR`] keeps the peak allocation inside it.

use std::io::Write;
use std::str::FromStr;

use age::armor::{ArmoredWriter, Format};
use age::secrecy::SecretString;

/// Lowest accepted scrypt work factor (`N = 2^10`). Below this the passphrase
/// stretching is too cheap to be worth doing at all.
pub const MIN_WORK_FACTOR: u8 = 10;
/// Highest accepted scrypt work factor. `N = 2^15` needs `128 * 8 * 2^15` = 32 MiB of
/// scratch memory; the block runs in a 64 MiB sandbox, so anything larger traps.
pub const MAX_WORK_FACTOR: u8 = 15;
/// Default scrypt work factor (`N = 2^14`, 16 MiB of scratch, a few hundred ms).
pub const DEFAULT_WORK_FACTOR: u8 = 14;
/// Largest plaintext accepted, in bytes. Armor inflates the output ~1.4x and the
/// whole file is buffered in memory alongside the scrypt scratch buffer.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// How the file key is wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// scrypt stanza — decrypt with the same passphrase.
    Passphrase,
    /// X25519 stanzas — decrypt with a matching `AGE-SECRET-KEY-1…` identity.
    Recipients,
}

impl Mode {
    /// Parses the `mode` param. Empty falls back to `passphrase`; a few obvious
    /// synonyms are accepted so a chat model doesn't have to guess the exact token.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "passphrase" | "password" | "scrypt" => Ok(Mode::Passphrase),
            "recipients" | "recipient" | "x25519" | "keys" | "key" => Ok(Mode::Recipients),
            other => Err(format!(
                "unknown mode '{other}' (use 'passphrase' or 'recipients')"
            )),
        }
    }

    /// The canonical param value.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Passphrase => "passphrase",
            Mode::Recipients => "recipients",
        }
    }
}

/// What an encryption produced, for callers that want more than the armor.
#[derive(Debug, Clone)]
pub struct Encrypted {
    /// The ASCII-armored age file.
    pub armored: String,
    /// The mode actually used.
    pub mode: Mode,
    /// How many X25519 recipients the file is encrypted to (0 in passphrase mode).
    pub recipients: usize,
    /// The scrypt work factor used (`None` in recipients mode).
    pub work_factor: Option<u8>,
}

/// Splits a recipient blob into individual keys.
///
/// Accepts the shapes people actually paste: one key per line (an `age` recipients
/// file), several on one line, or a comma-separated list. `#` starts a comment to end
/// of line, so a recipients file with headers pastes in unedited.
pub fn parse_recipient_list(keys: &str) -> Vec<String> {
    keys.lines()
        .map(|line| line.split('#').next().unwrap_or(""))
        .flat_map(|line| line.split([',', ';', ' ', '\t']))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Turns one pasted token into an age recipient, with an error that says what the
/// token actually looked like — the three near-misses below are what people paste.
fn parse_recipient(token: &str) -> Result<age::x25519::Recipient, String> {
    if token.starts_with("AGE-SECRET-KEY-1") || token.starts_with("age-secret-key-1") {
        return Err(
            "that is an age SECRET key (identity), not a recipient — share and paste the \
             matching public key, which starts with 'age1'"
                .into(),
        );
    }
    if token.starts_with("ssh-") || token.starts_with("ecdsa-sha2-") {
        return Err(format!(
            "'{}…' is an SSH public key; this tool encrypts to native age recipients only \
             (an 'age1…' key)",
            &token[..token.len().min(16)]
        ));
    }
    if token.starts_with("age1") && token.len() < 20 {
        return Err(format!(
            "recipient '{token}' is too short to be a complete age1 key"
        ));
    }
    age::x25519::Recipient::from_str(token).map_err(|e| {
        format!(
            "'{}…' is not a valid age recipient: {e} (expected a bech32 key starting with 'age1')",
            &token[..token.len().min(16)]
        )
    })
}

/// Encrypts `text` and returns the ASCII-armored age file.
///
/// `mode` picks the recipient type; `passphrase` is used by [`Mode::Passphrase`] and
/// `recipients` by [`Mode::Recipients`] — the unused one is ignored, so a page can keep
/// both fields filled while switching modes.
pub fn encrypt(
    text: &str,
    mode: &str,
    passphrase: &str,
    recipients: &str,
    work_factor: u8,
) -> Result<Encrypted, String> {
    let mode = Mode::parse(mode)?;

    if text.is_empty() {
        return Err("no text to encrypt".into());
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "text is {} bytes; this tool encrypts up to {} bytes ({} KiB) at a time — \
             encrypt larger data with the age CLI",
            text.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / 1024
        ));
    }

    match mode {
        Mode::Passphrase => {
            if passphrase.trim().is_empty() {
                return Err("passphrase is empty — enter the passphrase to encrypt with".into());
            }
            if !(MIN_WORK_FACTOR..=MAX_WORK_FACTOR).contains(&work_factor) {
                return Err(format!(
                    "work_factor must be between {MIN_WORK_FACTOR} and {MAX_WORK_FACTOR} \
                     (it is log2 of the scrypt cost N; {MAX_WORK_FACTOR} already needs 32 MiB \
                     of memory to decrypt), got {work_factor}"
                ));
            }
            let mut recipient =
                age::scrypt::Recipient::new(SecretString::from(passphrase.to_string()));
            recipient.set_work_factor(work_factor);
            let armored = armor_to_string(text, &recipient)?;
            Ok(Encrypted {
                armored,
                mode,
                recipients: 0,
                work_factor: Some(work_factor),
            })
        }
        Mode::Recipients => {
            let tokens = parse_recipient_list(recipients);
            if tokens.is_empty() {
                return Err(
                    "no recipients — paste at least one age public key (it starts with 'age1'), \
                     one per line for several"
                        .into(),
                );
            }
            let parsed = tokens
                .iter()
                .map(|t| parse_recipient(t))
                .collect::<Result<Vec<_>, _>>()?;
            let refs: Vec<&dyn age::Recipient> =
                parsed.iter().map(|r| r as &dyn age::Recipient).collect();
            let encryptor = age::Encryptor::with_recipients(refs.into_iter())
                .map_err(|e| format!("could not encrypt to those recipients: {e}"))?;
            let armored = armor_encryptor(text, encryptor)?;
            Ok(Encrypted {
                armored,
                mode,
                recipients: parsed.len(),
                work_factor: None,
            })
        }
    }
}

/// Convenience wrapper returning just the armor — what both the page and the CLI show.
pub fn run(
    text: &str,
    mode: &str,
    passphrase: &str,
    recipients: &str,
    work_factor: u8,
) -> Result<String, String> {
    encrypt(text, mode, passphrase, recipients, work_factor).map(|e| e.armored)
}

fn armor_to_string(text: &str, recipient: &dyn age::Recipient) -> Result<String, String> {
    let encryptor = age::Encryptor::with_recipients(std::iter::once(recipient))
        .map_err(|e| format!("could not build the age header: {e}"))?;
    armor_encryptor(text, encryptor)
}

fn armor_encryptor(text: &str, encryptor: age::Encryptor) -> Result<String, String> {
    let mut out: Vec<u8> = Vec::with_capacity(text.len() + 512);
    {
        let armor = ArmoredWriter::wrap_output(&mut out, Format::AsciiArmor)
            .map_err(|e| format!("armor error: {e}"))?;
        let mut writer = encryptor
            .wrap_output(armor)
            .map_err(|e| format!("could not write the age header: {e}"))?;
        writer
            .write_all(text.as_bytes())
            .map_err(|e| format!("could not encrypt the text: {e}"))?;
        // BOTH finishes are required: the stream writer flushes the last chunk +
        // its authentication tag, the armor writer flushes the trailing base64 and
        // the END line. Skipping either yields a truncated, undecryptable file.
        writer
            .finish()
            .map_err(|e| format!("could not finish the age stream: {e}"))?
            .finish()
            .map_err(|e| format!("could not finish the armor: {e}"))?;
    }
    String::from_utf8(out).map_err(|_| "age produced non-UTF-8 armor".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use age::secrecy::{ExposeSecret, SecretString};
    use std::io::Read;

    const ARMOR_BEGIN: &str = "-----BEGIN AGE ENCRYPTED FILE-----";
    const ARMOR_END: &str = "-----END AGE ENCRYPTED FILE-----";

    // A throwaway identity, generated in-test so no secret key is committed.
    fn identity() -> age::x25519::Identity {
        age::x25519::Identity::generate()
    }

    fn decrypt_with(armor: &str, id: &dyn age::Identity) -> Result<String, String> {
        let d = age::Decryptor::new_buffered(age::armor::ArmoredReader::new(armor.as_bytes()))
            .map_err(|e| e.to_string())?;
        let mut r = d.decrypt(std::iter::once(id)).map_err(|e| e.to_string())?;
        let mut s = String::new();
        r.read_to_string(&mut s).map_err(|e| e.to_string())?;
        Ok(s)
    }

    #[test]
    fn passphrase_round_trips() {
        let out = encrypt("attack at dawn", "passphrase", "correct horse", "", 10).unwrap();
        assert!(out.armored.starts_with(ARMOR_BEGIN), "{}", out.armored);
        assert!(out.armored.trim_end().ends_with(ARMOR_END));
        assert_eq!(out.mode, Mode::Passphrase);
        assert_eq!(out.work_factor, Some(10));
        assert_eq!(out.recipients, 0);

        let mut id = age::scrypt::Identity::new(SecretString::from("correct horse".to_string()));
        id.set_max_work_factor(MAX_WORK_FACTOR);
        assert_eq!(decrypt_with(&out.armored, &id).unwrap(), "attack at dawn");
    }

    #[test]
    fn wrong_passphrase_does_not_decrypt() {
        let out = run("attack at dawn", "passphrase", "correct horse", "", 10).unwrap();
        let mut id = age::scrypt::Identity::new(SecretString::from("wrong horse".to_string()));
        id.set_max_work_factor(MAX_WORK_FACTOR);
        assert!(decrypt_with(&out, &id).is_err());
    }

    #[test]
    fn same_input_encrypts_differently_each_time() {
        let a = run("hello", "passphrase", "pw", "", 10).unwrap();
        let b = run("hello", "passphrase", "pw", "", 10).unwrap();
        assert_ne!(a, b, "a fresh salt + file key must make every run unique");
    }

    #[test]
    fn recipient_round_trips() {
        let id = identity();
        let out = encrypt(
            "secret note",
            "recipients",
            "",
            &id.to_public().to_string(),
            14,
        )
        .unwrap();
        assert!(out.armored.starts_with(ARMOR_BEGIN));
        assert_eq!(out.mode, Mode::Recipients);
        assert_eq!(out.recipients, 1);
        assert_eq!(out.work_factor, None);
        assert_eq!(decrypt_with(&out.armored, &id).unwrap(), "secret note");
    }

    #[test]
    fn several_recipients_can_each_decrypt() {
        let (a, b) = (identity(), identity());
        // Comment line + comma separator + newline: the three shapes people paste.
        let keys = format!("# team keys\n{}, \n{}\n", a.to_public(), b.to_public());
        let out = encrypt("shared", "recipients", "", &keys, 14).unwrap();
        assert_eq!(out.recipients, 2);
        assert_eq!(decrypt_with(&out.armored, &a).unwrap(), "shared");
        assert_eq!(decrypt_with(&out.armored, &b).unwrap(), "shared");
    }

    #[test]
    fn other_identity_cannot_decrypt() {
        let out = run(
            "secret",
            "recipients",
            "",
            &identity().to_public().to_string(),
            14,
        )
        .unwrap();
        assert!(decrypt_with(&out, &identity()).is_err());
    }

    #[test]
    fn armor_is_wrapped_ascii() {
        let out = run("x".repeat(500).as_str(), "passphrase", "pw", "", 10).unwrap();
        assert!(out.is_ascii());
        assert!(
            out.lines().all(|l| l.len() <= 64),
            "armor lines must stay within the 64-char PEM width"
        );
    }

    #[test]
    fn unicode_survives_the_round_trip() {
        let id = identity();
        let out = run(
            "héllo 🌍 — naïve",
            "recipients",
            "",
            &id.to_public().to_string(),
            14,
        )
        .unwrap();
        assert_eq!(decrypt_with(&out, &id).unwrap(), "héllo 🌍 — naïve");
    }

    #[test]
    fn empty_text_is_rejected() {
        let e = run("", "passphrase", "pw", "", 14).unwrap_err();
        assert!(e.contains("no text"), "{e}");
    }

    #[test]
    fn oversized_text_is_rejected() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let e = run(&big, "passphrase", "pw", "", 14).unwrap_err();
        assert!(e.contains("up to"), "{e}");
    }

    #[test]
    fn empty_passphrase_is_rejected() {
        let e = run("hi", "passphrase", "   ", "", 14).unwrap_err();
        assert!(e.contains("passphrase is empty"), "{e}");
    }

    #[test]
    fn work_factor_out_of_range_is_rejected() {
        let e = run("hi", "passphrase", "pw", "", 20).unwrap_err();
        assert!(e.contains("work_factor must be between"), "{e}");
        let e = run("hi", "passphrase", "pw", "", 2).unwrap_err();
        assert!(e.contains("work_factor must be between"), "{e}");
    }

    #[test]
    fn missing_recipients_is_rejected() {
        let e = run("hi", "recipients", "pw", "  \n# only a comment\n", 14).unwrap_err();
        assert!(e.contains("no recipients"), "{e}");
    }

    #[test]
    fn a_secret_key_pasted_as_a_recipient_says_so() {
        let id = identity();
        let e = run("hi", "recipients", "", id.to_string().expose_secret(), 14).unwrap_err();
        assert!(e.contains("SECRET key"), "{e}");
    }

    #[test]
    fn an_ssh_key_is_rejected_by_name() {
        let e = run(
            "hi",
            "recipients",
            "",
            "ssh-ed25519 AAAAC3NzaC1lZDI1 me@host",
            14,
        )
        .unwrap_err();
        assert!(e.contains("SSH public key"), "{e}");
    }

    #[test]
    fn a_garbage_recipient_is_rejected() {
        let e = run("hi", "recipients", "", "age1notarealkeyatall", 14).unwrap_err();
        assert!(e.contains("not a valid age recipient"), "{e}");
    }

    #[test]
    fn unknown_mode_is_rejected() {
        let e = run("hi", "sideways", "pw", "", 14).unwrap_err();
        assert!(e.contains("unknown mode"), "{e}");
    }

    #[test]
    fn mode_synonyms_and_defaults_resolve() {
        assert_eq!(Mode::parse("").unwrap(), Mode::Passphrase);
        assert_eq!(Mode::parse(" Passphrase ").unwrap(), Mode::Passphrase);
        assert_eq!(Mode::parse("password").unwrap(), Mode::Passphrase);
        assert_eq!(Mode::parse("RECIPIENTS").unwrap(), Mode::Recipients);
        assert_eq!(Mode::parse("x25519").unwrap(), Mode::Recipients);
        assert_eq!(Mode::Recipients.as_str(), "recipients");
    }

    #[test]
    fn recipient_list_splits_on_the_shapes_people_paste() {
        let list = parse_recipient_list("age1aaa, age1bbb\nage1ccc age1ddd # trailing note\n\n");
        assert_eq!(list, vec!["age1aaa", "age1bbb", "age1ccc", "age1ddd"]);
    }
}
