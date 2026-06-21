//! gizza-ai/hex-view core — render bytes as a classic `xxd`-style hex dump:
//! an 8-digit hex offset column, the bytes in hex (grouped 8+8 with a gap), and
//! an ASCII gutter. Pure-Rust, dependency-free.

/// Render `data` as a hex dump with `bytes_per_line` bytes per row (clamped to
/// 1..=64; a gap is inserted at the halfway point, like xxd's 8+8 default).
pub fn hex_dump(data: &[u8], bytes_per_line: usize) -> String {
    let per = bytes_per_line.clamp(1, 64);
    let half = per / 2;
    let mut out = String::new();

    for (row, chunk) in data.chunks(per).enumerate() {
        let offset = row * per;
        out.push_str(&format!("{offset:08x}  "));

        // Hex columns.
        for i in 0..per {
            if i == half && half != 0 {
                out.push(' '); // extra gap between the two halves
            }
            match chunk.get(i) {
                Some(b) => out.push_str(&format!("{b:02x} ")),
                None => out.push_str("   "), // pad missing bytes on the last row
            }
        }

        // ASCII gutter.
        out.push_str(" |");
        for &b in chunk {
            let c = if (0x20..=0x7e).contains(&b) { b as char } else { '.' };
            out.push(c);
        }
        out.push('|');
        out.push('\n');
    }

    if data.is_empty() {
        out.push_str("00000000\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_layout() {
        let d = b"Hello world!\n";
        let dump = hex_dump(d, 16);
        // offset, hex of 'H'=48, ASCII gutter present
        assert!(dump.starts_with("00000000  48 65 6c 6c 6f 20 77 6f  72 6c 64 21 0a"));
        assert!(dump.contains("|Hello world!.|"));
    }

    #[test]
    fn multiple_rows_and_offsets() {
        let d: Vec<u8> = (0u8..=31).collect();
        let dump = hex_dump(&d, 16);
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("00000000  00 01 02 03"));
        assert!(lines[1].starts_with("00000010  10 11 12 13"));
    }

    #[test]
    fn last_row_padding_aligns_gutter() {
        let d = b"abc"; // 3 bytes, 16/line → padded
        let dump = hex_dump(d, 16);
        assert!(dump.contains("61 62 63 "));
        assert!(dump.trim_end().ends_with("|abc|"));
    }

    #[test]
    fn non_printable_becomes_dot() {
        let dump = hex_dump(&[0x00, 0xff, 0x41, 0x7f], 16);
        assert!(dump.contains("|..A.|"));
    }

    #[test]
    fn empty_input() {
        assert_eq!(hex_dump(b"", 16), "00000000\n");
    }

    #[test]
    fn custom_width() {
        let dump = hex_dump(b"abcd", 4);
        let lines: Vec<&str> = dump.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("|abcd|"));
    }
}
