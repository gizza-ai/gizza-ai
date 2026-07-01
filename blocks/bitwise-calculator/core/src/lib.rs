//! bitwise-calculator core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! One bitwise operation on integers at a fixed bit width (8/16/32/64):
//! `and`/`or`/`xor` (two operands), `not`/`popcount` (one operand), and
//! `shl`/`shr`/`rotl`/`rotr` (operand + count). Operands accept decimal, hex
//! (`0x`), binary (`0b`) and octal (`0o`) with `_`/space digit separators;
//! a leading `-` is read as two's complement at the chosen width. The result
//! is rendered in binary (nibble-grouped), octal, decimal, hex and signed
//! two's complement.

/// Widths the tool supports; `bits = ""` falls back to [`DEFAULT_BITS`].
pub const WIDTHS: [u32; 4] = [8, 16, 32, 64];
pub const DEFAULT_BITS: u32 = 32;
pub const OPS: [&str; 9] = [
    "and", "or", "xor", "not", "shl", "shr", "rotl", "rotr", "popcount",
];

fn mask_for(bits: u32) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn radix_name(radix: u32) -> &'static str {
    match radix {
        2 => "binary",
        8 => "octal",
        16 => "hex",
        _ => "decimal",
    }
}

/// Parse one operand into its two's-complement value at `bits` width.
/// Accepts `0b`/`0o`/`0x` prefixes (else decimal), `_`/space separators and a
/// leading `-`/`+`. Errors name the operand and say what was expected.
fn parse_operand(name: &str, raw: &str, bits: u32) -> Result<u64, String> {
    let (magnitude, negative) = parse_magnitude(name, raw)?;
    let mask = mask_for(bits);
    let unsigned_max = mask as u128;
    let signed_min_mag = 1u128 << (bits - 1);
    let range_err = || {
        format!(
            "operand '{name}' = {} does not fit in {bits} bits (unsigned range 0..{unsigned_max}, signed range -{signed_min_mag}..{})",
            raw.trim(),
            signed_min_mag - 1
        )
    };
    if negative {
        if magnitude > signed_min_mag {
            return Err(range_err());
        }
        if magnitude == 0 {
            return Ok(0);
        }
        Ok((((1u128 << bits) - magnitude) as u64) & mask)
    } else {
        if magnitude > unsigned_max {
            return Err(range_err());
        }
        Ok(magnitude as u64)
    }
}

/// Parse a shift/rotate count: same formats as an operand but must be ≥ 0.
/// Counts larger than any width are fine (shifts saturate to 0, rotates wrap).
fn parse_count(name: &str, raw: &str) -> Result<u64, String> {
    let (magnitude, negative) = parse_magnitude(name, raw)?;
    if negative && magnitude > 0 {
        return Err(format!(
            "shift/rotate count '{name}' must be a non-negative integer (got '{}')",
            raw.trim()
        ));
    }
    Ok(magnitude.min(u64::MAX as u128) as u64)
}

/// Shared digit parsing: sign, optional base prefix, separators. Returns the
/// magnitude (saturated just past u64 so range errors stay friendly) and sign.
fn parse_magnitude(name: &str, raw: &str) -> Result<(u128, bool), String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(format!("operand '{name}' is empty"));
    }
    let (negative, rest) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let lower = rest.trim_start().to_ascii_lowercase();
    let (radix, digits) = if let Some(d) = lower.strip_prefix("0b") {
        (2u32, d)
    } else if let Some(d) = lower.strip_prefix("0o") {
        (8, d)
    } else if let Some(d) = lower.strip_prefix("0x") {
        (16, d)
    } else {
        (10, lower.as_str())
    };
    let mut acc: u128 = 0;
    let mut seen = false;
    // One past u64::MAX — big enough to stay "out of every width's range".
    const CAP: u128 = 1u128 << 64;
    for c in digits.chars() {
        if c == '_' || c == ' ' {
            continue;
        }
        let d = c.to_digit(radix).ok_or_else(|| {
            format!(
                "operand '{name}': '{c}' is not a valid {} digit in '{s}'",
                radix_name(radix)
            )
        })?;
        seen = true;
        if acc <= CAP {
            acc = acc.saturating_mul(radix as u128).saturating_add(d as u128);
        }
    }
    if !seen {
        return Err(format!(
            "operand '{name}': no digits found in '{s}' (expected e.g. 87, 0x57, 0b0101_0111 or 0o127)"
        ));
    }
    Ok((acc, negative))
}

/// Zero-padded binary of `v` at `bits` width, grouped in nibbles ("0100 0101").
fn bin_grouped(v: u64, bits: u32) -> String {
    let mut out = String::with_capacity(bits as usize + bits as usize / 4);
    for i in (0..bits).rev() {
        out.push(if (v >> i) & 1 == 1 { '1' } else { '0' });
        if i != 0 && i % 4 == 0 {
            out.push(' ');
        }
    }
    out
}

/// Two's-complement (signed) reading of `v` at `bits` width.
fn signed_of(v: u64, bits: u32) -> i64 {
    if bits == 64 {
        v as i64
    } else if v >= 1u64 << (bits - 1) {
        v as i64 - (1i64 << bits)
    } else {
        v as i64
    }
}

fn render(op_echo: &str, v: u64, bits: u32) -> String {
    format!(
        "operation: {op_echo} ({bits}-bit)\nbinary   : {}\noctal    : 0o{v:o}\ndecimal  : {v}\nhex      : 0x{v:0width$x}\nsigned   : {}",
        bin_grouped(v, bits),
        signed_of(v, bits),
        width = (bits / 4) as usize
    )
}

/// The tool entrypoint, shared by chat, CLI and the web page.
/// `op = ""` defaults to `and`; `bits = ""` defaults to 32.
pub fn compute(a: &str, op: &str, b: &str, bits: &str) -> Result<String, String> {
    let bits: u32 = match bits.trim() {
        "" => DEFAULT_BITS,
        t => t
            .parse()
            .ok()
            .filter(|w| WIDTHS.contains(w))
            .ok_or_else(|| format!("bits must be one of 8, 16, 32, 64 (got '{t}')"))?,
    };
    let op_norm = op.trim().to_ascii_lowercase();
    let op = if op_norm.is_empty() { "and" } else { op_norm.as_str() };
    if !OPS.contains(&op) {
        return Err(format!("op must be one of {} (got '{op}')", OPS.join(", ")));
    }
    if a.trim().is_empty() {
        return Err("operand 'a' is required".into());
    }
    let av = parse_operand("a", a, bits)?;
    let a_echo = a.trim();
    let mask = mask_for(bits);
    match op {
        "and" | "or" | "xor" => {
            if b.trim().is_empty() {
                return Err(format!("op '{op}' needs a second operand 'b'"));
            }
            let bv = parse_operand("b", b, bits)?;
            let v = match op {
                "and" => av & bv,
                "or" => av | bv,
                _ => av ^ bv,
            };
            Ok(render(
                &format!("{a_echo} {} {}", op.to_uppercase(), b.trim()),
                v,
                bits,
            ))
        }
        "not" => Ok(render(&format!("NOT {a_echo}"), !av & mask, bits)),
        "shl" | "shr" => {
            if b.trim().is_empty() {
                return Err(format!("op '{op}' needs a shift count in 'b'"));
            }
            let count = parse_count("b", b)?;
            let v = if count >= bits as u64 {
                0 // logical shift: every bit falls off the edge
            } else if op == "shl" {
                (av << count) & mask
            } else {
                av >> count
            };
            Ok(render(
                &format!("{a_echo} {} {}", op.to_uppercase(), b.trim()),
                v,
                bits,
            ))
        }
        "rotl" | "rotr" => {
            if b.trim().is_empty() {
                return Err(format!("op '{op}' needs a rotate count in 'b'"));
            }
            let count = parse_count("b", b)?;
            let k = (count % bits as u64) as u32;
            let v = if k == 0 {
                av
            } else if op == "rotl" {
                ((av << k) | (av >> (bits - k))) & mask
            } else {
                ((av >> k) | (av << (bits - k))) & mask
            };
            Ok(render(
                &format!("{a_echo} {} {}", op.to_uppercase(), b.trim()),
                v,
                bits,
            ))
        }
        _ => {
            // popcount — the count is the result; also show the masked input.
            Ok(format!(
                "operation: POPCOUNT {a_echo} ({bits}-bit)\ninput    : {}\nset bits : {}",
                bin_grouped(av, bits),
                av.count_ones()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn and_8bit_matches_worked_example() {
        // 87 = 0101 0111, 101 = 0110 0101 → AND = 0100 0101 = 69 = 0x45
        let out = compute("87", "and", "101", "8").unwrap();
        assert_eq!(
            out,
            "operation: 87 AND 101 (8-bit)\nbinary   : 0100 0101\noctal    : 0o105\ndecimal  : 69\nhex      : 0x45\nsigned   : 69"
        );
    }

    #[test]
    fn or_and_xor_work() {
        assert!(compute("0b1100", "or", "0b1010", "8")
            .unwrap()
            .contains("decimal  : 14"));
        assert!(compute("0b1100", "xor", "0b1010", "8")
            .unwrap()
            .contains("decimal  : 6"));
    }

    #[test]
    fn hex_octal_binary_prefixes_and_separators_parse() {
        let out = compute("0x57", "and", "0b0110_0101", "8").unwrap();
        assert!(out.contains("hex      : 0x45"), "{out}");
        let out = compute("0o127", "and", "0x 65", "8").unwrap();
        assert!(out.contains("decimal  : 69"), "{out}");
    }

    #[test]
    fn not_masks_to_width_and_shows_signed() {
        let out = compute("0x0F", "not", "", "8").unwrap();
        assert!(out.contains("binary   : 1111 0000"), "{out}");
        assert!(out.contains("decimal  : 240"), "{out}");
        assert!(out.contains("signed   : -16"), "{out}");
    }

    #[test]
    fn shifts_are_logical_and_saturate_past_the_width() {
        assert!(compute("0b0001", "shl", "3", "8").unwrap().contains("decimal  : 8"));
        assert!(compute("0b1000", "shr", "3", "8").unwrap().contains("decimal  : 1"));
        // shifting by >= width drops every bit
        assert!(compute("255", "shl", "8", "8").unwrap().contains("decimal  : 0"));
        assert!(compute("255", "shr", "100", "8").unwrap().contains("decimal  : 0"));
    }

    #[test]
    fn rotates_wrap_modulo_the_width() {
        let out = compute("0b1000_0001", "rotl", "1", "8").unwrap();
        assert!(out.contains("binary   : 0000 0011"), "{out}");
        let out = compute("0b1000_0001", "rotr", "9", "8").unwrap(); // 9 % 8 == 1
        assert!(out.contains("binary   : 1100 0000"), "{out}");
        // a count that is a multiple of the width is a no-op
        assert!(compute("0xAB", "rotl", "16", "8").unwrap().contains("hex      : 0xab"));
    }

    #[test]
    fn popcount_counts_set_bits() {
        let out = compute("0xDEADBEEF", "popcount", "", "32").unwrap();
        assert!(out.contains("set bits : 24"), "{out}");
        assert!(compute("0", "popcount", "", "8").unwrap().contains("set bits : 0"));
    }

    #[test]
    fn negative_decimal_is_twos_complement() {
        let out = compute("-8", "and", "0xFF", "8").unwrap();
        assert!(out.contains("binary   : 1111 1000"), "{out}");
        assert!(out.contains("signed   : -8"), "{out}");
        // -128 is the signed minimum for 8 bits
        assert!(compute("-128", "or", "0", "8").unwrap().contains("decimal  : 128"));
    }

    #[test]
    fn defaults_are_and_and_32_bit() {
        let out = compute("6", "", "3", "").unwrap();
        assert!(out.contains("operation: 6 AND 3 (32-bit)"), "{out}");
        assert!(out.contains("decimal  : 2"), "{out}");
        assert!(
            out.contains("binary   : 0000 0000 0000 0000 0000 0000 0000 0010"),
            "{out}"
        );
    }

    #[test]
    fn full_64_bit_width_works() {
        let out = compute("0xFFFF_FFFF_FFFF_FFFF", "not", "", "64").unwrap();
        assert!(out.contains("decimal  : 0"), "{out}");
        let out = compute("0xFFFF_FFFF_FFFF_FFFF", "popcount", "", "64").unwrap();
        assert!(out.contains("set bits : 64"), "{out}");
        // signed reading of all-ones at 64 bits is -1
        let out = compute("0", "not", "", "64").unwrap();
        assert!(out.contains("signed   : -1"), "{out}");
    }

    #[test]
    fn operand_that_does_not_fit_the_width_errors() {
        let err = compute("300", "not", "", "8").unwrap_err();
        assert!(err.contains("does not fit in 8 bits"), "{err}");
        assert!(err.contains("0..255"), "{err}");
        let err = compute("-129", "not", "", "8").unwrap_err();
        assert!(err.contains("does not fit in 8 bits"), "{err}");
        // way past u64 must error, not overflow
        let err = compute("0x1_0000_0000_0000_0000_0000", "not", "", "64").unwrap_err();
        assert!(err.contains("does not fit in 64 bits"), "{err}");
    }

    #[test]
    fn invalid_digits_and_empty_operands_error() {
        let err = compute("0b102", "not", "", "8").unwrap_err();
        assert!(err.contains("not a valid binary digit"), "{err}");
        let err = compute("0x", "not", "", "8").unwrap_err();
        assert!(err.contains("no digits"), "{err}");
        let err = compute("", "and", "1", "8").unwrap_err();
        assert!(err.contains("'a' is required"), "{err}");
        let err = compute("1", "and", "", "8").unwrap_err();
        assert!(err.contains("needs a second operand 'b'"), "{err}");
        let err = compute("1", "shl", "", "8").unwrap_err();
        assert!(err.contains("needs a shift count"), "{err}");
    }

    #[test]
    fn bad_op_bits_and_negative_count_error() {
        let err = compute("1", "nand", "1", "8").unwrap_err();
        assert!(err.contains("op must be one of"), "{err}");
        let err = compute("1", "and", "1", "12").unwrap_err();
        assert!(err.contains("bits must be one of 8, 16, 32, 64"), "{err}");
        let err = compute("1", "shl", "-2", "8").unwrap_err();
        assert!(err.contains("non-negative"), "{err}");
    }
}
