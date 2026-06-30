//! strip-ansi-codes core — remove ANSI escape / color codes from terminal text.
//! Pure compute, no deps; shared by the chat skill block and the web page.
//!
//! A hand-rolled ECMA-48 scanner (no regex/crate) walks the bytes and drops the
//! 7-bit escape sequences a terminal emits: SGR colour/style codes (`\x1b[31m`),
//! cursor / erase control (`\x1b[2J`, `\x1b[H`), OSC strings (window titles,
//! hyperlinks, terminated by BEL or ST), and DCS/SOS/PM/APC strings. All escape
//! bytes are ASCII (`< 0x80`), so scanning bytes never splits a multi-byte UTF-8
//! character — the surrounding text (including emoji / accents) is preserved.

const ESC: u8 = 0x1B; // \e — start of every 7-bit escape sequence
const BEL: u8 = 0x07; // \a — an alternate OSC string terminator (xterm)

/// Remove ANSI escape sequences from `text`.
///
/// `scope` selects what to strip:
/// - `""` | `"all"` (default): every escape sequence — colours, cursor/erase
///   control, OSC/DCS strings. The result is plain readable text.
/// - `"color"`: only SGR colour & style codes (`CSI … m`, e.g. `\x1b[1;31m`).
///   Cursor movement, erase, OSC and other sequences are left untouched — useful
///   when you want to drop colour but keep the layout control intact.
///
/// Returns `Err` on an unknown `scope`. The output is always valid UTF-8.
pub fn strip(text: &str, scope: &str) -> Result<String, String> {
    let only_color = match scope {
        "" | "all" => false,
        "color" => true,
        other => {
            return Err(format!(
                "invalid scope {other:?}: expected \"all\" or \"color\""
            ))
        }
    };

    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b != ESC {
            out.push(b);
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            // A trailing lone ESC is not a complete sequence — drop it so the
            // result matches a mid-string lone ESC (also dropped) and stays clean.
            i += 1;
            continue;
        }

        match bytes[i + 1] {
            // CSI — Control Sequence Introducer (`ESC [`). The SGR colour codes
            // live here (final byte `m`); so do cursor/erase codes (`H`, `J`, …).
            b'[' => {
                let (end, final_byte) = scan_csi(bytes, i + 2);
                let is_sgr = final_byte == Some(b'm');
                if only_color && !is_sgr {
                    // Keep a non-colour control sequence verbatim: emit the ESC
                    // and rescan the rest as ordinary bytes.
                    out.push(b);
                    i += 1;
                } else {
                    i = end;
                }
            }
            // OSC — Operating System Command (`ESC ]`), e.g. window-title or
            // hyperlink strings, terminated by BEL or ST (`ESC \`).
            b']' if !only_color => i = scan_string(bytes, i + 2),
            // DCS / SOS / PM / APC string sequences, terminated by ST.
            b'P' | b'X' | b'^' | b'_' if !only_color => i = scan_string(bytes, i + 2),
            // Any other `ESC <Fe/nF>` sequence (charset select `ESC ( B`,
            // reset `ESC c`, keypad modes, …): a run of intermediate bytes then
            // a single final byte.
            _ if !only_color => i = scan_esc(bytes, i + 1),
            // In colour-only mode every non-SGR escape is preserved verbatim.
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }

    // `out` is a subset of valid-UTF-8 `text` with only ASCII bytes removed, so
    // it is still valid UTF-8; the check is a cheap guarantee, never an error.
    String::from_utf8(out).map_err(|e| format!("internal: produced invalid UTF-8 ({e})"))
}

/// Scan a CSI body starting at `start` (just after `ESC [`): parameter bytes
/// `0x30..=0x3F`, then intermediate bytes `0x20..=0x2F`, then one final byte
/// `0x40..=0x7E`. Returns `(index past the sequence, final byte if present)`.
/// A truncated sequence (EOF before a final byte) consumes what was scanned.
fn scan_csi(bytes: &[u8], start: usize) -> (usize, Option<u8>) {
    let mut i = start;
    while i < bytes.len() && (0x30..=0x3F).contains(&bytes[i]) {
        i += 1;
    }
    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
        i += 1;
    }
    if i < bytes.len() && (0x40..=0x7E).contains(&bytes[i]) {
        (i + 1, Some(bytes[i]))
    } else {
        (i, None)
    }
}

/// Scan a string-type sequence (OSC/DCS/SOS/PM/APC) from `start` to its
/// terminator — BEL (`0x07`) or ST (`ESC \`) — returning the index past it.
/// An unterminated string runs to end of input.
fn scan_string(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == BEL {
            return i + 1;
        }
        if bytes[i] == ESC && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
            return i + 2;
        }
        i += 1;
    }
    i
}

/// Scan a generic `ESC <Fe/nF>` sequence from `start` (just after `ESC`):
/// optional intermediate bytes `0x20..=0x2F` then one final byte `0x30..=0x7E`.
/// Returns the index past the sequence; a lone trailing `ESC` consumes nothing
/// extra (the caller already advanced past the ESC byte).
fn scan_esc(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
        i += 1;
    }
    if i < bytes.len() && (0x30..=0x7E).contains(&bytes[i]) {
        i + 1
    } else {
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_basic_sgr_color() {
        assert_eq!(
            strip("\x1b[31mred\x1b[0m text", "all").unwrap(),
            "red text"
        );
    }

    #[test]
    fn strips_multi_parameter_sgr() {
        assert_eq!(
            strip("\x1b[1;38;5;202mbright\x1b[0m", "").unwrap(),
            "bright"
        );
    }

    #[test]
    fn strips_cursor_and_erase_in_all_mode() {
        // \x1b[2J clear screen, \x1b[H home, \x1b[K erase line.
        assert_eq!(
            strip("\x1b[2J\x1b[Hhome\x1b[Kend", "all").unwrap(),
            "homeend"
        );
    }

    #[test]
    fn color_scope_keeps_cursor_codes_but_drops_color() {
        // Only the SGR (…m) sequences go; \x1b[2J and \x1b[H stay.
        assert_eq!(
            strip("\x1b[2J\x1b[31mred\x1b[0m\x1b[H", "color").unwrap(),
            "\x1b[2Jred\x1b[H"
        );
    }

    #[test]
    fn strips_osc_hyperlink_and_title() {
        // OSC 8 hyperlink (ST-terminated) wrapping link text, and an OSC 0
        // window-title (BEL-terminated).
        let input = "\x1b]0;my title\x07\x1b]8;;https://x.com\x1b\\link\x1b]8;;\x1b\\";
        assert_eq!(strip(input, "all").unwrap(), "link");
    }

    #[test]
    fn strips_generic_escape_sequences() {
        // Charset select `ESC ( B`, full reset `ESC c`.
        assert_eq!(strip("\x1b(Bplain\x1bc", "all").unwrap(), "plain");
    }

    #[test]
    fn preserves_plain_and_unicode_text() {
        assert_eq!(
            strip("café ☕ — no codes here\n", "all").unwrap(),
            "café ☕ — no codes here\n"
        );
    }

    #[test]
    fn preserves_unicode_adjacent_to_codes() {
        // Multi-byte chars next to escape bytes must survive intact.
        assert_eq!(
            strip("\x1b[32m✓ café\x1b[0m", "all").unwrap(),
            "✓ café"
        );
    }

    #[test]
    fn handles_truncated_sequence_at_eof() {
        // A dangling, unfinished CSI is dropped rather than emitted raw.
        assert_eq!(strip("done\x1b[", "all").unwrap(), "done");
        assert_eq!(strip("done\x1b[1;", "all").unwrap(), "done");
    }

    #[test]
    fn lone_escape_at_end_is_dropped() {
        assert_eq!(strip("text\x1b", "all").unwrap(), "text");
    }

    #[test]
    fn defaults_to_all_when_scope_blank() {
        assert_eq!(strip("\x1b[31mx\x1b[0m", "").unwrap(), "x");
    }

    #[test]
    fn rejects_unknown_scope() {
        let err = strip("x", "colours").unwrap_err();
        assert!(err.contains("invalid scope"), "got: {err}");
    }

    #[test]
    fn empty_input_yields_empty() {
        assert_eq!(strip("", "all").unwrap(), "");
    }
}
