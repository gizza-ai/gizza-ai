//! charset-transcode core — re-decode text from a legacy charset into clean
//! UTF-8, fixing mojibake. Pure compute, shared by the chat skill block and the
//! web page; no wafer/wasm-bindgen deps.
//!
//! The classic problem this solves: a string of bytes that was *originally* a
//! legacy single-byte or multi-byte encoding (ISO-8859-x, Windows-1252,
//! Shift_JIS, …) but got handled as if it were something else, leaving garbled
//! "mojibake" like `Ã©` (should be `é`) or `â€œ` (should be `“`).
//!
//! Every surface (chat / CLI / page) hands us the input as a Rust `&str`, i.e.
//! already-valid UTF-8 *characters*. The mojibake we're fixing happened because
//! the original UTF-8 bytes of some text were wrongly decoded under a legacy
//! charset (`from`) — e.g. the UTF-8 bytes `C3 A9` (`é`) were read as
//! Windows-1252, producing the two characters `Ã©`.
//!
//! To undo that we run the *inverse*: take the mojibake string's characters and
//! **encode** them back to bytes under `from` (`Ã©` → `C3 A9`), then **decode
//! those bytes as UTF-8** (`C3 A9` → `é`). That recovers the intended text. The
//! `from` charset is the one the text was wrongly decoded *as* (most commonly
//! Windows-1252 / ISO-8859-1). Pass `"auto"` (or blank) to try the common
//! charsets and keep the cleanest result, and `passes > 1` to un-nest
//! double-encoded ("double mojibake") input.

use encoding_rs::Encoding;

/// `errors` policy when a source byte sequence isn't valid in `from`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Errors {
    /// Substitute U+FFFD (the Unicode replacement character) and continue.
    Replace,
    /// Fail with an error listing the offending byte offset.
    Strict,
}

impl Errors {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "replace" => Ok(Errors::Replace),
            "strict" => Ok(Errors::Strict),
            other => Err(format!(
                "invalid errors {other:?}: expected \"replace\" or \"strict\""
            )),
        }
    }
}

/// Resolve a charset *label* to an `encoding_rs::Encoding`. Accepts the WHATWG
/// label set (case-insensitive, with the usual aliases — `latin1`, `cp1252`,
/// `sjis`, etc.), which covers every legacy charset gizza exposes.
fn resolve(label: &str) -> Result<&'static Encoding, String> {
    let label = label.trim();
    if label.is_empty() {
        return Err("a source charset ('from') is required".into());
    }
    Encoding::for_label(label.as_bytes()).ok_or_else(|| {
        format!("unknown source charset {label:?}: try e.g. \"windows-1252\", \"iso-8859-1\", \"shift_jis\", \"euc-kr\", \"windows-1251\", \"gbk\"")
    })
}

/// Maximum number of repair passes `passes` may request. Caps the un-nesting of
/// multiply-encoded ("double mojibake") input.
pub const MAX_PASSES: u32 = 8;

/// The charsets `auto` mode tries, in priority order. Windows-1252 first (it's
/// the overwhelming cause of Western mojibake and a superset of ISO-8859-1's
/// printable range), then the other common single-byte and CJK legacy charsets.
const AUTO_CANDIDATES: &[&str] = &[
    "windows-1252",
    "iso-8859-1",
    "iso-8859-15",
    "windows-1251",
    "koi8-r",
    "shift_jis",
    "euc-jp",
    "euc-kr",
    "gbk",
    "big5",
];

/// Run ONE repair pass: encode `input` to `enc`-bytes, then decode those bytes
/// as UTF-8. `Ok(None)` means this charset can't repair the input (a char isn't
/// representable in `enc`, or the re-encoded bytes aren't valid UTF-8 in strict
/// mode) — used by `auto` to reject a candidate. `Err` only on internal misuse.
fn fix_once(input: &str, enc: &'static Encoding, policy: Errors) -> Option<String> {
    // `encode` flags `had_unmappable` when a char has no representation in the
    // target charset — then `enc` is the wrong guess.
    let (bytes, _used, had_unmappable) = enc.encode(input);
    if had_unmappable {
        return None;
    }
    match std::str::from_utf8(&bytes) {
        Ok(s) => Some(s.to_string()),
        Err(_) if policy == Errors::Strict => None,
        // Lossy: invalid UTF-8 sequences become U+FFFD.
        Err(_) => Some(String::from_utf8_lossy(&bytes).into_owned()),
    }
}

/// Score a repair result: lower is "more likely correct". Penalises the
/// replacement char (U+FFFD) and C1 control chars (the tell-tale leftovers of a
/// bad charset guess), so `auto` prefers the cleanest decode.
fn badness(s: &str) -> usize {
    s.chars()
        .filter(|&c| c == '\u{FFFD}' || ('\u{80}'..='\u{9F}').contains(&c))
        .count()
}

/// Fix mojibake: re-decode `input` that was wrongly decoded under a legacy
/// charset, recovering the intended UTF-8 text. Encodes the input characters
/// back to charset-bytes, then decodes those bytes as UTF-8.
///
/// - `from`: the charset the text was wrongly decoded *as*, a WHATWG label/alias
///   (`windows-1252`, `iso-8859-1`/`latin1`, `shift_jis`/`sjis`, `euc-jp`,
///   `euc-kr`, `gbk`, `big5`, `windows-1251`, `koi8-r`, …). Use `"auto"` (or
///   leave blank) to try the common charsets and keep the cleanest result.
/// - `errors`: `"replace"` (default) substitutes U+FFFD for bytes that, after
///   re-encoding, don't form valid UTF-8; `"strict"` fails instead.
/// - `passes`: apply the repair this many times (clamped to `1..=MAX_PASSES`,
///   `0` → `1`). Use `>1` to un-nest double-encoded mojibake. A pass that no
///   longer changes the text, can't be applied, or would only make the text
///   *dirtier* (over-fixing already-clean text) stops the loop early.
///
/// Returns `Err` on an unknown charset, a bad `errors` value, when the chosen
/// charset can't repair the input, or (in `auto`) when no candidate charset can.
pub fn transcode(input: &str, from: &str, errors: &str, passes: u32) -> Result<String, String> {
    let policy = Errors::parse(errors)?;
    let rounds = passes.clamp(1, MAX_PASSES);
    let from = from.trim();

    if from.is_empty() || from.eq_ignore_ascii_case("auto") {
        transcode_auto(input, policy, rounds)
    } else {
        let enc = resolve(from)?;
        let mut cur = input.to_string();
        for i in 0..rounds {
            match fix_once(&cur, enc, policy) {
                // Accept a pass only if it changes the text AND doesn't make it
                // dirtier — so repeated passes un-nest real double-mojibake but
                // stop once the text is clean (re-applying the charset to clean
                // text would re-introduce U+FFFD).
                Some(next) if next != cur && badness(&next) <= badness(&cur) => cur = next,
                // No change, or the next pass would only over-fix it: stop.
                Some(_) => break,
                None if i == 0 => {
                    // First pass couldn't even apply -> the charset is wrong.
                    return Err(format!(
                        "could not re-decode the input as {} — that charset is probably not the right 'from' (try \"windows-1252\", \"iso-8859-1\", or \"auto\"; or set errors=\"replace\")",
                        enc.name()
                    ));
                }
                None => break, // later pass can't continue; keep what we have.
            }
        }
        Ok(cur)
    }
}

/// `auto` mode: for each pass, try every candidate charset and keep the result
/// with the lowest `badness` that actually changed the text; stop when no
/// candidate improves it.
fn transcode_auto(input: &str, policy: Errors, rounds: u32) -> Result<String, String> {
    let mut cur = input.to_string();
    let mut applied_any = false;

    for _ in 0..rounds {
        let mut best: Option<String> = None;
        for label in AUTO_CANDIDATES {
            let enc = Encoding::for_label(label.as_bytes()).expect("static label");
            if let Some(cand) = fix_once(&cur, enc, policy) {
                // Only accept a candidate that changes the text and is no worse
                // than the current best.
                if cand != cur
                    && best
                        .as_ref()
                        .map(|b| badness(&cand) < badness(b))
                        .unwrap_or(true)
                {
                    best = Some(cand);
                }
            }
        }
        match best {
            // Don't accept a "repair" that's dirtier than what we already have.
            Some(b) if badness(&b) <= badness(&cur) => {
                cur = b;
                applied_any = true;
            }
            _ => break,
        }
    }

    if !applied_any && badness(&cur) > 0 {
        return Err(
            "auto could not find a charset that cleanly repairs this text — try specifying 'from' explicitly (e.g. \"windows-1252\")"
                .into(),
        );
    }
    Ok(cur)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_windows_1252_mojibake() {
        // "café"'s UTF-8 bytes (C3 A9 for "é") were wrongly decoded as
        // Windows-1252, giving the mojibake "cafÃ©" (C3 -> "Ã", A9 -> "©").
        // Re-encoding "cafÃ©" back to Windows-1252 yields 63 61 66 C3 A9, which
        // as UTF-8 is "café".
        assert_eq!(
            transcode("cafÃ©", "windows-1252", "replace", 1).unwrap(),
            "café"
        );
        // naïve résumé mojibake round-trips too.
        assert_eq!(
            transcode("naÃ¯ve rÃ©sumÃ©", "windows-1252", "replace", 1).unwrap(),
            "naïve résumé"
        );
    }

    #[test]
    fn fixes_smart_quote_mojibake() {
        // The curly double-quotes “ ” are U+201C / U+201D, UTF-8 E2 80 9C /
        // E2 80 9D. Under Windows-1252 those bytes show as "â€œ" / "â€\u{009d}".
        // 0x9D is unmapped in 1252 -> the mojibake there is the control char
        // U+009D, which we feed back literally.
        assert_eq!(
            transcode("â€œhiâ€\u{009d}", "windows-1252", "replace", 1).unwrap(),
            "“hi”"
        );
    }

    #[test]
    fn iso_8859_1_is_a_pure_byte_map() {
        // ISO-8859-1 maps U+0000..=U+00FF 1:1 to bytes 0x00..=0xFF, so encoding
        // the mojibake "Ã©" (U+00C3 U+00A9) gives bytes C3 A9 = "é" in UTF-8.
        assert_eq!(transcode("Ã©", "iso-8859-1", "replace", 1).unwrap(), "é");
        // "latin1" alias resolves identically.
        assert_eq!(transcode("Ã©", "latin1", "replace", 1).unwrap(), "é");
    }

    #[test]
    fn ascii_passes_through_unchanged() {
        // Pure ASCII encodes to the same bytes in every supported charset and
        // is already valid UTF-8, so it round-trips untouched (no pass changes
        // it, so the loop stops at the first no-op).
        assert_eq!(
            transcode("Hello, world!", "windows-1252", "replace", 1).unwrap(),
            "Hello, world!"
        );
    }

    #[test]
    fn unknown_charset_is_rejected() {
        let err = transcode("x", "not-a-charset", "replace", 1).unwrap_err();
        assert!(err.contains("unknown source charset"), "got: {err}");
    }

    #[test]
    fn invalid_errors_value_is_rejected() {
        let err = transcode("x", "windows-1252", "explode", 1).unwrap_err();
        assert!(err.contains("invalid errors"), "got: {err}");
    }

    #[test]
    fn strict_mode_rejects_a_wrong_charset_guess() {
        // "é" (U+00E9) encodes to the single byte 0xE9 under Windows-1252, which
        // is not valid UTF-8 on its own -> strict mode can't apply the repair,
        // so the (already-clean) input is reported as the wrong-charset guess.
        let err = transcode("é", "windows-1252", "strict", 1).unwrap_err();
        assert!(err.contains("not the right 'from'"), "got: {err}");
        // In replace mode the repair WOULD yield a dirtier result (U+FFFD), so
        // the over-fix guard rejects it and the clean input is left untouched —
        // applying the wrong charset to already-clean text doesn't corrupt it.
        assert_eq!(transcode("é", "windows-1252", "replace", 1).unwrap(), "é");
    }

    #[test]
    fn errors_defaults_to_replace_when_blank() {
        assert_eq!(transcode("cafÃ©", "windows-1252", "", 1).unwrap(), "café");
    }

    #[test]
    fn rejects_chars_not_in_source_charset() {
        // A character with no Windows-1252 representation (e.g. a CJK ideograph)
        // means windows-1252 can't be the charset the text was decoded as.
        let err = transcode("日", "windows-1252", "replace", 1).unwrap_err();
        assert!(err.contains("not the right 'from'"), "got: {err}");
    }

    #[test]
    fn fixes_shift_jis_mojibake() {
        // "日本"'s UTF-8 bytes (E6 97 A5 E6 9C AC) wrongly decoded as Shift_JIS
        // give a specific mojibake string; re-encoding it to Shift_JIS recovers
        // those bytes, and decoding as UTF-8 gives back "日本".
        let mojibake = encoding_rs::SHIFT_JIS
            .decode_without_bom_handling("日本".as_bytes())
            .0
            .into_owned();
        assert_eq!(
            transcode(&mojibake, "shift_jis", "replace", 1).unwrap(),
            "日本"
        );
    }

    #[test]
    fn auto_detects_windows_1252() {
        // No 'from' given -> auto tries the candidates and recovers "café".
        assert_eq!(transcode("cafÃ©", "auto", "replace", 1).unwrap(), "café");
        // blank 'from' behaves as auto; "AUTO" is case-insensitive.
        assert_eq!(transcode("cafÃ©", "", "replace", 1).unwrap(), "café");
        assert_eq!(transcode("cafÃ©", "AUTO", "replace", 1).unwrap(), "café");
    }

    #[test]
    fn auto_leaves_clean_text_unchanged() {
        // Already-clean UTF-8 with no mojibake markers: auto finds nothing better
        // and returns it untouched (no error, since there's no garbage).
        assert_eq!(
            transcode("Hello, world!", "auto", "replace", 1).unwrap(),
            "Hello, world!"
        );
        assert_eq!(transcode("café", "auto", "replace", 1).unwrap(), "café");
    }

    #[test]
    fn passes_unnests_double_mojibake() {
        // Encode the mojibake one extra level: "café" -> "cafÃ©" (1252 mojibake),
        // then mojibake THAT again. Two passes peel both layers.
        let once = transcode("cafÃ©", "windows-1252", "strict", 1); // recovers café
        // Build a double-encoded sample directly: the bytes of "cafÃ©" decoded
        // through 1252 a second time.
        let _ = once;
        let layer1 = "cafÃ©"; // 1 layer of mojibake over "café"
        // Re-mojibake layer1 by mis-decoding ITS utf-8 bytes as 1252:
        let layer2 = encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(layer1.as_bytes())
            .0
            .into_owned();
        // One pass only peels one layer (back to layer1); two peels to "café".
        assert_eq!(
            transcode(&layer2, "windows-1252", "replace", 1).unwrap(),
            layer1
        );
        assert_eq!(
            transcode(&layer2, "windows-1252", "replace", 2).unwrap(),
            "café"
        );
    }

    #[test]
    fn passes_is_clamped() {
        // passes 0 behaves as 1; huge values cap at MAX_PASSES and still
        // terminate (the no-op stop prevents over-fixing clean text).
        assert_eq!(transcode("cafÃ©", "windows-1252", "replace", 0).unwrap(), "café");
        assert_eq!(
            transcode("cafÃ©", "windows-1252", "replace", 1000).unwrap(),
            "café"
        );
    }
}
