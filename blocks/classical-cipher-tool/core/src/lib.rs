//! classical-cipher-tool core — encrypt / decrypt with the classic pre-modern
//! ciphers: Caesar (shift), Vigenère (keyword), Atbash (alphabet mirror), and
//! Rail-fence (transposition). Plus a Caesar brute-force that returns all 26
//! candidate shifts at once. No wafer/wasm-bindgen deps — shared by the chat
//! skill block and the web page.
//!
//! All substitution ciphers operate on ASCII letters only; case is preserved and
//! every non-letter (digits, spaces, punctuation) passes through unchanged. This
//! is the conventional behaviour for these classical ciphers and makes them
//! lossless on mixed text. Rail-fence transposes the whole string (spaces
//! included).

/// Maximum number of rails the rail-fence cipher accepts. A fence with more
/// rails than characters is pointless (each char gets its own rail), so we cap
/// at a generous value to keep argv/UI bounded and reject absurd input.
pub const MAX_RAILS: u32 = 64;

/// Apply a classical cipher.
///
/// - `cipher` (`"caesar"` | `"vigenere"` | `"atbash"` | `"rail-fence"`): which
///   cipher to use (blank → `"caesar"`).
/// - `operation` (`"encrypt"` | `"decrypt"` | `"brute-force"`, blank →
///   `"encrypt"`): direction. `"brute-force"` is only valid with `caesar` and
///   returns all 26 candidate shifts as `shift NN: <text>` lines so you can
///   eyeball the plaintext.
/// - `key`: cipher-specific —
///     - caesar: the shift amount, an integer (negative allowed; reduced mod 26).
///       Blank → 3 (the classic Caesar shift). Ignored for brute-force.
///     - vigenere: the keyword (letters only; non-letters in the key are ignored).
///       Required and must contain at least one letter.
///     - atbash: ignored (Atbash is keyless / self-inverse).
///     - rail-fence: the number of rails, an integer `2..=MAX_RAILS`. Blank → 3.
///
/// Returns `Err` on an unknown cipher/operation or an invalid key for the
/// chosen cipher.
pub fn run(cipher: &str, operation: &str, key: &str, text: &str) -> Result<String, String> {
    let cipher = if cipher.is_empty() { "caesar" } else { cipher };
    let op = Operation::parse(operation)?;

    match cipher {
        "caesar" => caesar_dispatch(op, key, text),
        "vigenere" => {
            let decrypt = op.transposition_direction("vigenere")?;
            vigenere(text, key, decrypt)
        }
        "atbash" => {
            // Atbash is its own inverse, so encrypt == decrypt; brute-force is meaningless.
            if matches!(op, Operation::BruteForce) {
                return Err("brute-force is only supported for the caesar cipher (Atbash has no key)".into());
            }
            Ok(atbash(text))
        }
        "rail-fence" => {
            let decrypt = op.transposition_direction("rail-fence")?;
            let rails = parse_rails(key)?;
            Ok(rail_fence(text, rails, decrypt))
        }
        other => Err(format!(
            "invalid cipher {other:?}: expected \"caesar\", \"vigenere\", \"atbash\", or \"rail-fence\""
        )),
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Encrypt,
    Decrypt,
    BruteForce,
}

impl Operation {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "encrypt" => Ok(Operation::Encrypt),
            "decrypt" => Ok(Operation::Decrypt),
            "brute-force" | "bruteforce" | "brute_force" => Ok(Operation::BruteForce),
            other => Err(format!(
                "invalid operation {other:?}: expected \"encrypt\", \"decrypt\", or \"brute-force\""
            )),
        }
    }

    /// Map a non-brute-force op to a decrypt flag, rejecting brute-force for a
    /// cipher that doesn't support it.
    fn transposition_direction(self, cipher: &str) -> Result<bool, String> {
        match self {
            Operation::Encrypt => Ok(false),
            Operation::Decrypt => Ok(true),
            Operation::BruteForce => Err(format!(
                "brute-force is only supported for the caesar cipher (not {cipher})"
            )),
        }
    }
}

// ---- Caesar ---------------------------------------------------------------

fn caesar_dispatch(op: Operation, key: &str, text: &str) -> Result<String, String> {
    match op {
        Operation::BruteForce => Ok(caesar_brute_force(text)),
        Operation::Encrypt | Operation::Decrypt => {
            let shift = parse_shift(key)?;
            let effective = if matches!(op, Operation::Decrypt) {
                -shift
            } else {
                shift
            };
            Ok(caesar(text, effective))
        }
    }
}

/// Parse the Caesar shift. Blank → 3 (the classic shift). Reduced mod 26.
fn parse_shift(key: &str) -> Result<i32, String> {
    let k = key.trim();
    if k.is_empty() {
        return Ok(3);
    }
    k.parse::<i32>()
        .map(|n| n.rem_euclid(26))
        .map_err(|_| format!("invalid caesar shift {key:?}: expected a whole number"))
}

/// Shift every ASCII letter by `shift` positions (mod 26), preserving case;
/// non-letters pass through. `shift` may be negative or out of range — it is
/// normalised mod 26.
fn caesar(text: &str, shift: i32) -> String {
    let s = shift.rem_euclid(26) as u8;
    text.chars().map(|c| shift_letter(c, s)).collect()
}

fn shift_letter(c: char, shift: u8) -> char {
    if c.is_ascii_uppercase() {
        (((c as u8 - b'A' + shift) % 26) + b'A') as char
    } else if c.is_ascii_lowercase() {
        (((c as u8 - b'a' + shift) % 26) + b'a') as char
    } else {
        c
    }
}

/// Return all 26 Caesar decryptions, one `shift NN: <text>` line per shift
/// (0..=25). Shift 0 is the unchanged input; the intended plaintext is whichever
/// line reads naturally.
fn caesar_brute_force(text: &str) -> String {
    (0..26)
        .map(|shift| {
            // Decrypt = shift the ciphertext back by `shift`.
            let plain = caesar(text, -(shift as i32));
            format!("shift {shift:>2}: {plain}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---- Vigenère -------------------------------------------------------------

fn vigenere(text: &str, key: &str, decrypt: bool) -> Result<String, String> {
    // Keep only the alphabetic key letters (the standard convention: spaces /
    // punctuation in a passphrase are ignored).
    let key_shifts: Vec<u8> = key
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase() as u8 - b'a')
        .collect();
    if key_shifts.is_empty() {
        return Err("vigenere requires a key containing at least one letter".into());
    }

    let mut ki = 0usize;
    let out = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                let mut shift = key_shifts[ki % key_shifts.len()];
                if decrypt {
                    shift = (26 - shift) % 26;
                }
                ki += 1;
                shift_letter(c, shift)
            } else {
                c
            }
        })
        .collect();
    Ok(out)
}

// ---- Atbash ---------------------------------------------------------------

/// Atbash maps A↔Z, B↔Y, … — a self-inverse alphabet mirror. Non-letters pass through.
fn atbash(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                (b'Z' - (c as u8 - b'A')) as char
            } else if c.is_ascii_lowercase() {
                (b'z' - (c as u8 - b'a')) as char
            } else {
                c
            }
        })
        .collect()
}

// ---- Rail fence -----------------------------------------------------------

fn parse_rails(key: &str) -> Result<usize, String> {
    let k = key.trim();
    let n: u32 = if k.is_empty() {
        3
    } else {
        k.parse()
            .map_err(|_| format!("invalid rail count {key:?}: expected a whole number"))?
    };
    if n < 2 || n > MAX_RAILS {
        return Err(format!(
            "invalid rail count {n}: rail-fence needs between 2 and {MAX_RAILS} rails"
        ));
    }
    Ok(n as usize)
}

/// Rail-fence (zig-zag) transposition cipher over the WHOLE string (spaces and
/// punctuation are transposed too, unlike the substitution ciphers). With 1
/// rail it would be a no-op, so `rails >= 2` is enforced by `parse_rails`.
fn rail_fence(text: &str, rails: usize, decrypt: bool) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 || rails <= 1 {
        return text.to_string();
    }
    // Compute the rail index for each position (the zig-zag pattern).
    let pattern = rail_pattern(n, rails);
    if !decrypt {
        // Encrypt: read rail by rail.
        let mut out = String::with_capacity(n);
        for r in 0..rails {
            for (i, &rail) in pattern.iter().enumerate() {
                if rail == r {
                    out.push(chars[i]);
                }
            }
        }
        out
    } else {
        // Decrypt: distribute the ciphertext back across the rails, then read in
        // zig-zag order.
        let mut out = vec!['\0'; n];
        let mut ci = chars.iter();
        for r in 0..rails {
            for (i, &rail) in pattern.iter().enumerate() {
                if rail == r {
                    if let Some(&c) = ci.next() {
                        out[i] = c;
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

/// The rail index visited at each of `n` positions for a `rails`-rail fence:
/// 0,1,2,…,rails-1,rails-2,…,1,0,1,… (bounces between the top and bottom rail).
fn rail_pattern(n: usize, rails: usize) -> Vec<usize> {
    let mut pattern = Vec::with_capacity(n);
    let mut rail = 0i64;
    let mut dir = 1i64;
    for _ in 0..n {
        pattern.push(rail as usize);
        if rail == 0 {
            dir = 1;
        } else if rail as usize == rails - 1 {
            dir = -1;
        }
        rail += dir;
    }
    pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caesar_encrypts_with_default_shift_3() {
        // Blank key → classic shift of 3. Case preserved, non-letters untouched.
        assert_eq!(run("caesar", "encrypt", "", "Hello, World!").unwrap(), "Khoor, Zruog!");
    }

    #[test]
    fn caesar_round_trips() {
        let ct = run("caesar", "encrypt", "13", "Attack at dawn").unwrap();
        assert_eq!(ct, "Nggnpx ng qnja");
        assert_eq!(run("caesar", "decrypt", "13", &ct).unwrap(), "Attack at dawn");
    }

    #[test]
    fn caesar_negative_and_wrapping_shift() {
        // Shift of -1 (== 25) and a >26 shift both reduce mod 26.
        assert_eq!(run("caesar", "encrypt", "-1", "ABC").unwrap(), "ZAB");
        assert_eq!(run("caesar", "encrypt", "29", "ABC").unwrap(), "DEF");
    }

    #[test]
    fn caesar_brute_force_lists_all_26_shifts() {
        let out = run("caesar", "brute-force", "", "Khoor").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 26);
        // shift 0 is the unchanged ciphertext.
        assert_eq!(lines[0], "shift  0: Khoor");
        // shift 3 recovers "Hello".
        assert!(lines.iter().any(|l| *l == "shift  3: Hello"), "got:\n{out}");
    }

    #[test]
    fn vigenere_round_trips_and_ignores_non_letters() {
        // Classic textbook example: ATTACKATDAWN / LEMON -> LXFOPVEFRNHR.
        let ct = run("vigenere", "encrypt", "LEMON", "ATTACKATDAWN").unwrap();
        assert_eq!(ct, "LXFOPVEFRNHR");
        assert_eq!(run("vigenere", "decrypt", "LEMON", &ct).unwrap(), "ATTACKATDAWN");
        // Non-letters in the plaintext do not consume a key letter.
        let ct2 = run("vigenere", "encrypt", "key", "a b!c").unwrap();
        assert_eq!(run("vigenere", "decrypt", "key", &ct2).unwrap(), "a b!c");
    }

    #[test]
    fn vigenere_rejects_keyless() {
        let err = run("vigenere", "encrypt", "123 ...", "hello").unwrap_err();
        assert!(err.contains("at least one letter"), "got: {err}");
    }

    #[test]
    fn atbash_is_self_inverse() {
        assert_eq!(run("atbash", "encrypt", "", "Hello").unwrap(), "Svool");
        assert_eq!(run("atbash", "decrypt", "", "Svool").unwrap(), "Hello");
        assert_eq!(run("atbash", "encrypt", "", "abcXYZ 123").unwrap(), "zyxCBA 123");
    }

    #[test]
    fn rail_fence_round_trips() {
        // Classic 3-rail example: WEAREDISCOVEREDFLEEATONCE.
        let ct = run("rail-fence", "encrypt", "3", "WEAREDISCOVEREDFLEEATONCE").unwrap();
        assert_eq!(ct, "WECRLTEERDSOEEFEAOCAIVDEN");
        assert_eq!(
            run("rail-fence", "decrypt", "3", &ct).unwrap(),
            "WEAREDISCOVEREDFLEEATONCE"
        );
    }

    #[test]
    fn rail_fence_default_rails_and_spaces_transposed() {
        // Spaces are part of the transposition, so round-trip must preserve them.
        let pt = "the quick brown fox";
        let ct = run("rail-fence", "encrypt", "", pt).unwrap();
        assert_eq!(run("rail-fence", "decrypt", "", &ct).unwrap(), pt);
    }

    #[test]
    fn rail_fence_rejects_bad_rail_count() {
        assert!(run("rail-fence", "encrypt", "1", "abc").unwrap_err().contains("between 2"));
        assert!(run("rail-fence", "encrypt", "999", "abc").unwrap_err().contains("between 2"));
        assert!(run("rail-fence", "encrypt", "two", "abc").unwrap_err().contains("whole number"));
    }

    #[test]
    fn defaults_cipher_to_caesar_when_blank() {
        assert_eq!(run("", "encrypt", "1", "abc").unwrap(), "bcd");
    }

    #[test]
    fn rejects_unknown_cipher_and_operation() {
        assert!(run("enigma", "encrypt", "", "x").unwrap_err().contains("invalid cipher"));
        assert!(run("caesar", "scramble", "", "x").unwrap_err().contains("invalid operation"));
    }

    #[test]
    fn brute_force_rejected_for_non_caesar() {
        assert!(run("vigenere", "brute-force", "k", "x").unwrap_err().contains("only supported for the caesar"));
        assert!(run("atbash", "brute-force", "", "x").unwrap_err().contains("only supported for the caesar"));
        assert!(run("rail-fence", "brute-force", "3", "x").unwrap_err().contains("only supported for the caesar"));
    }

    #[test]
    fn caesar_invalid_shift() {
        assert!(run("caesar", "encrypt", "abc", "x").unwrap_err().contains("expected a whole number"));
    }

    #[test]
    fn empty_text_is_fine() {
        assert_eq!(run("caesar", "encrypt", "5", "").unwrap(), "");
        assert_eq!(run("rail-fence", "encrypt", "3", "").unwrap(), "");
        assert_eq!(run("atbash", "encrypt", "", "").unwrap(), "");
    }
}
