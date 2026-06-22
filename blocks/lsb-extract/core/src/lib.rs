//! lsb-extract core — pure-Rust extraction of LSB (least-significant-bit)
//! steganography payloads hidden in an image's pixel channels.
//!
//! Two extraction modes:
//!
//! * **auto** — look for the `gizza-ai/lsb-embed` wire format. Bits are read
//!   MSB-first from the low `bits_per_channel` bits of the R, G, B channels (a
//!   skipped), trying depths 1..=4. The format is:
//!     `[MAGIC "LSB1" (4 B)] [bits_per_channel (1 B)] [length N (4 B, BE)] [N payload bytes]`
//!   so the extractor learns the depth + exact length and returns the payload.
//!
//! * **raw** — no header. Read the low `bits_per_channel` bits of the selected
//!   channels in row-major pixel order, pack them MSB-first into bytes, and
//!   return the resulting stream (capped to a sane size). This recovers payloads
//!   embedded by other LSB tools that use a fixed channel set + depth and no
//!   length header — the user picks the channels and depth.
//!
//! Pure (no wafer/wasm deps): unit-testable on the host, reused by the chat
//! skill block. File-input → text/JSON output → chat + CLI only, no page.

use image::RgbaImage;

const MAGIC: &[u8; 4] = b"LSB1";
const HEADER_LEN: usize = 9; // MAGIC(4) + bpc(1) + len(4)

/// Channel order is always R,G,B,A as indices 0..=3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Channels {
    mask: u8, // bit i set => include channel i (R=0,G=1,B=2,A=3)
}

impl Channels {
    /// Parse a channel spec like "rgb", "r", "rg", "rgba", "a" (case-insensitive,
    /// order-insensitive, duplicates ignored). Empty or unknown letters error.
    pub fn parse(spec: &str) -> Result<Channels, String> {
        let mut mask = 0u8;
        for c in spec.trim().chars() {
            let bit = match c.to_ascii_lowercase() {
                'r' => 0,
                'g' => 1,
                'b' => 2,
                'a' => 3,
                ' ' | ',' => continue,
                other => {
                    return Err(format!(
                        "unknown channel '{other}' — use any of r, g, b, a (e.g. \"rgb\")"
                    ))
                }
            };
            mask |= 1 << bit;
        }
        if mask == 0 {
            return Err("channels must name at least one of r, g, b, a (e.g. \"rgb\")".into());
        }
        Ok(Channels { mask })
    }

    /// The included channel indices in fixed R,G,B,A order.
    fn indices(&self) -> impl Iterator<Item = usize> + '_ {
        (0..4usize).filter(move |i| self.mask & (1 << i) != 0)
    }

    pub fn count(&self) -> usize {
        self.mask.count_ones() as usize
    }

    /// Lowercase label in fixed r,g,b,a order, e.g. "rgb".
    pub fn label(&self) -> String {
        let mut s = String::new();
        for (i, c) in ['r', 'g', 'b', 'a'].into_iter().enumerate() {
            if self.mask & (1u8 << i) != 0 {
                s.push(c);
            }
        }
        s
    }
}

/// Outcome of an extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    /// The recovered bytes.
    pub bytes: Vec<u8>,
    /// `bytes` decoded as UTF-8, if it is valid UTF-8.
    pub text: Option<String>,
    /// bits/channel that produced this result (the depth actually used).
    pub bits_per_channel: u8,
    /// In auto mode, whether a valid `LSB1` header was found.
    pub header_found: bool,
    /// Channels read (e.g. "rgb"); for auto mode this is always "rgb".
    pub channels: String,
}

/// Bit ordering within each channel's low-bit field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BitOrder {
    /// Most-significant bit of the field first (matches `gizza-ai/lsb-embed`).
    Msb,
    /// Least-significant bit of the field first (common in other encoders).
    Lsb,
}

impl BitOrder {
    pub fn parse(s: &str) -> Result<BitOrder, String> {
        match s.trim().to_lowercase().as_str() {
            "msb" => Ok(BitOrder::Msb),
            "lsb" => Ok(BitOrder::Lsb),
            other => Err(format!("bit_order must be \"msb\" or \"lsb\", got \"{other}\"")),
        }
    }
}

/// Raw-mode capacity is bounded by 8 bits/channel; auto mode stays 1..=4.
fn validate_bpc(bpc: u8) -> Result<u8, String> {
    if (1..=8).contains(&bpc) {
        Ok(bpc)
    } else {
        Err(format!("bits_per_channel must be 1..=8, got {bpc}"))
    }
}

/// Best-effort magic-byte sniff of extracted bytes — surfaces "looks like a PNG"
/// etc. when the payload is a file rather than UTF-8 text. Returns a short label
/// or `None`. Mirrors the common signatures a steganographer hides.
pub fn sniff_filetype(bytes: &[u8]) -> Option<&'static str> {
    let starts = |sig: &[u8]| bytes.len() >= sig.len() && &bytes[..sig.len()] == sig;
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some("PNG image")
    } else if starts(b"\xFF\xD8\xFF") {
        Some("JPEG image")
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some("GIF image")
    } else if starts(b"BM") {
        Some("BMP image")
    } else if starts(b"%PDF-") {
        Some("PDF document")
    } else if starts(b"PK\x03\x04") || starts(b"PK\x05\x06") {
        Some("ZIP archive (or docx/xlsx/jar)")
    } else if starts(b"\x1F\x8B") {
        Some("gzip data")
    } else if starts(b"7z\xBC\xAF\x27\x1C") {
        Some("7-Zip archive")
    } else if starts(b"Rar!\x1A\x07") {
        Some("RAR archive")
    } else if starts(b"\x00\x00\x01\x00") {
        Some("ICO image")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("WebP image")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Some("WAV audio")
    } else if starts(b"OggS") {
        Some("Ogg media")
    } else if starts(b"ID3") || starts(b"\xFF\xFB") {
        Some("MP3 audio")
    } else {
        None
    }
}

fn decode(image_bytes: &[u8]) -> Result<RgbaImage, String> {
    if image_bytes.is_empty() {
        return Err("empty image input".into());
    }
    let img = image::load_from_memory(image_bytes).map_err(|e| format!("decode image: {e}"))?;
    Ok(img.to_rgba8())
}

/// AUTO mode: find a `gizza-ai/lsb-embed` payload (RGB channels, MSB-first),
/// trying bit depths 1..=4. Returns the decoded payload, or an error if no valid
/// header is present (wrong image, or it was re-encoded lossily).
pub fn extract_auto(image_bytes: &[u8]) -> Result<Extracted, String> {
    let rgba = decode(image_bytes)?;
    let rgb = Channels::parse("rgb")?;
    for bpc in 1u8..=4 {
        let mut bits = BitCollector::new(&rgba, rgb, bpc, BitOrder::Msb);
        let Some(header) = bits.read_bytes(HEADER_LEN) else {
            continue;
        };
        if &header[0..4] != MAGIC || header[4] != bpc {
            continue;
        }
        let len = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) as usize;
        let Some(payload) = bits.read_bytes(len) else {
            continue;
        };
        let text = String::from_utf8(payload.clone()).ok();
        return Ok(Extracted {
            bytes: payload,
            text,
            bits_per_channel: bpc,
            header_found: true,
            channels: "rgb".into(),
        });
    }
    Err("no lsb-embed payload found — the image has no LSB1 header (wrong image, or it was re-encoded lossily, e.g. saved as JPEG). Try raw mode with explicit channels + bit depth.".into())
}

/// RAW mode: read the low `bits_per_channel` bits of the selected `channels`
/// (row-major pixel order, MSB-first), packing them into bytes. `max_bytes`
/// caps the output. No header — the whole capacity is dumped (truncated to the
/// cap). Use this for payloads embedded without an LSB1 length header.
pub fn extract_raw(
    image_bytes: &[u8],
    channels: Channels,
    bits_per_channel: u8,
    bit_order: BitOrder,
    invert: bool,
    max_bytes: usize,
) -> Result<Extracted, String> {
    let bpc = validate_bpc(bits_per_channel)?;
    let rgba = decode(image_bytes)?;
    let mut bits = BitCollector::new(&rgba, channels, bpc, bit_order);
    let total_bits = rgba.pixels().len() * channels.count() * bpc as usize;
    let cap = max_bytes.min(total_bits / 8);
    let mut bytes = bits.read_bytes(cap).unwrap_or_default();
    if invert {
        for b in &mut bytes {
            *b = !*b;
        }
    }
    let text = String::from_utf8(bytes.clone()).ok();
    Ok(Extracted {
        bytes,
        text,
        bits_per_channel: bpc,
        header_found: false,
        channels: channels.label(),
    })
}

/// Walks pixel channels (selectable subset of R,G,B,A in fixed order) pulling
/// `bpc` low bits from each, MSB-first, reassembling the embedded bit stream
/// back into bytes.
struct BitCollector<'a> {
    raw: &'a [u8], // RGBA8, 4 bytes per pixel
    order: Vec<usize>,
    bpc: u8,
    bit_order: BitOrder,
    px: usize,
    ch_pos: usize, // index into `order` for the current pixel
    sub: u8,
}

impl<'a> BitCollector<'a> {
    fn new(rgba: &'a RgbaImage, channels: Channels, bpc: u8, bit_order: BitOrder) -> Self {
        Self {
            raw: rgba.as_raw(),
            order: channels.indices().collect(),
            bpc,
            bit_order,
            px: 0,
            ch_pos: 0,
            sub: 0,
        }
    }

    fn next_bit(&mut self) -> Option<u8> {
        loop {
            let base = self.px * 4;
            if base + 4 > self.raw.len() {
                return None;
            }
            if self.ch_pos >= self.order.len() {
                self.px += 1;
                self.ch_pos = 0;
                self.sub = 0;
                continue;
            }
            let ch = self.order[self.ch_pos];
            let channel_val = self.raw[base + ch];
            // MSB-first reads the field's high bit first; LSB-first reads bit 0
            // first. `sub` counts 0..bpc within the field.
            let shift = match self.bit_order {
                BitOrder::Msb => self.bpc - 1 - self.sub,
                BitOrder::Lsb => self.sub,
            };
            let b = (channel_val >> shift) & 1;
            self.sub += 1;
            if self.sub >= self.bpc {
                self.sub = 0;
                self.ch_pos += 1;
            }
            return Some(b);
        }
    }

    fn read_bytes(&mut self, n: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(n);
        let mut acc = 0u8;
        let mut acc_bits = 0u8;
        for _ in 0..n {
            for _ in 0..8 {
                let bit = self.next_bit()?;
                acc = (acc << 1) | bit;
                acc_bits += 1;
                if acc_bits == 8 {
                    out.push(acc);
                    acc = 0;
                    acc_bits = 0;
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn test_carrier(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for (i, px) in img.pixels_mut().enumerate() {
            let v = (i % 200) as u8;
            *px = Rgba([v, 255 - v, v.wrapping_mul(3), 255]);
        }
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// Mirror lsb-embed's encoder (RGB, MSB-first, LSB1 header) so we can test
    /// auto-extraction without depending on the sibling crate.
    fn embed_lsb1(carrier: &[u8], payload: &[u8], bpc: u8) -> Vec<u8> {
        let img = image::load_from_memory(carrier).unwrap();
        let mut rgba = img.to_rgba8();
        let mut stream = Vec::new();
        stream.extend_from_slice(MAGIC);
        stream.push(bpc);
        stream.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        stream.extend_from_slice(payload);

        let mask = !((1u8 << bpc) - 1);
        let mut bit_idx = 0usize;
        let next = |i: usize| -> Option<u8> {
            let byte = i / 8;
            if byte >= stream.len() {
                return None;
            }
            Some((stream[byte] >> (7 - (i % 8))) & 1)
        };
        'outer: for px in rgba.pixels_mut() {
            for ch in 0..3 {
                let mut val = 0u8;
                let mut got = 0u8;
                for _ in 0..bpc {
                    match next(bit_idx) {
                        Some(b) => {
                            val = (val << 1) | b;
                            got += 1;
                            bit_idx += 1;
                        }
                        None => break,
                    }
                }
                if got == 0 {
                    break 'outer;
                }
                val <<= bpc - got;
                px.0[ch] = (px.0[ch] & mask) | val;
                if got < bpc {
                    break 'outer;
                }
            }
        }
        let mut out = Cursor::new(Vec::new());
        rgba.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn auto_round_trip_text() {
        let carrier = test_carrier(64, 64);
        let secret = "hello, hidden world! \u{1F511}".as_bytes();
        let stego = embed_lsb1(&carrier, secret, 1);
        let got = extract_auto(&stego).unwrap();
        assert_eq!(got.bytes, secret);
        assert_eq!(got.text.as_deref(), Some("hello, hidden world! \u{1F511}"));
        assert!(got.header_found);
        assert_eq!(got.bits_per_channel, 1);
        assert_eq!(got.channels, "rgb");
    }

    #[test]
    fn auto_round_trip_all_depths() {
        let carrier = test_carrier(64, 64);
        let secret = vec![0xABu8; 400];
        for bpc in 1..=4u8 {
            let stego = embed_lsb1(&carrier, &secret, bpc);
            let got = extract_auto(&stego).unwrap();
            assert_eq!(got.bytes, secret, "bpc {bpc}");
            assert_eq!(got.bits_per_channel, bpc);
        }
    }

    #[test]
    fn auto_non_utf8_payload_has_no_text() {
        let carrier = test_carrier(32, 32);
        let secret = vec![0xFFu8, 0xFE, 0x00, 0x80]; // invalid UTF-8
        let stego = embed_lsb1(&carrier, &secret, 1);
        let got = extract_auto(&stego).unwrap();
        assert_eq!(got.bytes, secret);
        assert_eq!(got.text, None);
    }

    #[test]
    fn auto_clean_image_errors() {
        let carrier = test_carrier(40, 40);
        let err = extract_auto(&carrier).unwrap_err();
        assert!(err.contains("no lsb-embed payload"), "got: {err}");
    }

    #[test]
    fn auto_empty_payload() {
        let carrier = test_carrier(16, 16);
        let stego = embed_lsb1(&carrier, b"", 1);
        let got = extract_auto(&stego).unwrap();
        assert!(got.bytes.is_empty());
        assert_eq!(got.text.as_deref(), Some(""));
    }

    #[test]
    fn raw_reads_lsb1_stream_with_header() {
        // raw mode over rgb @ 1 bit on an LSB1 stego image yields the raw stream,
        // which begins with the MAGIC bytes.
        let carrier = test_carrier(64, 64);
        let stego = embed_lsb1(&carrier, b"secret payload here", 1);
        let ch = Channels::parse("rgb").unwrap();
        let got = extract_raw(&stego, ch, 1, BitOrder::Msb, false, 64).unwrap();
        assert_eq!(&got.bytes[0..4], MAGIC);
        assert_eq!(got.channels, "rgb");
        assert!(!got.header_found);
    }

    #[test]
    fn raw_respects_max_bytes_cap() {
        let carrier = test_carrier(64, 64);
        let ch = Channels::parse("rgb").unwrap();
        let got = extract_raw(&carrier, ch, 1, BitOrder::Msb, false, 10).unwrap();
        assert_eq!(got.bytes.len(), 10);
    }

    #[test]
    fn raw_capacity_scales_with_channels_and_depth() {
        // 8x8 = 64 px. rgb@1 => 64*3*1 = 192 bits = 24 bytes. rgba@4 => 64*4*4 = 1024 bits = 128 bytes.
        let carrier = test_carrier(8, 8);
        let rgb1 =
            extract_raw(&carrier, Channels::parse("rgb").unwrap(), 1, BitOrder::Msb, false, 10_000)
                .unwrap();
        assert_eq!(rgb1.bytes.len(), 24);
        let rgba4 = extract_raw(
            &carrier,
            Channels::parse("rgba").unwrap(),
            4,
            BitOrder::Msb,
            false,
            10_000,
        )
        .unwrap();
        assert_eq!(rgba4.bytes.len(), 128);
    }

    #[test]
    fn raw_single_channel() {
        let carrier = test_carrier(16, 16); // 256 px * 1 ch * 1 bit = 32 bytes
        let got =
            extract_raw(&carrier, Channels::parse("g").unwrap(), 1, BitOrder::Msb, false, 10_000)
                .unwrap();
        assert_eq!(got.bytes.len(), 32);
        assert_eq!(got.channels, "g");
    }

    #[test]
    fn raw_lsb_first_differs_from_msb_and_invert_complements() {
        let carrier = test_carrier(32, 32);
        let ch = Channels::parse("rgb").unwrap();
        let msb = extract_raw(&carrier, ch, 2, BitOrder::Msb, false, 64).unwrap();
        let lsb = extract_raw(&carrier, ch, 2, BitOrder::Lsb, false, 64).unwrap();
        // At depth>1 the two bit orders read the same field differently → differ.
        assert_ne!(msb.bytes, lsb.bytes);
        // invert flips every bit of the recovered stream.
        let inv = extract_raw(&carrier, ch, 2, BitOrder::Msb, true, 64).unwrap();
        for (a, b) in msb.bytes.iter().zip(&inv.bytes) {
            assert_eq!(*a, !*b);
        }
    }

    #[test]
    fn raw_bit_depth_up_to_8() {
        let carrier = test_carrier(8, 8); // 64 px * 3 ch * 8 bit = 1536 bits = 192 bytes
        let got =
            extract_raw(&carrier, Channels::parse("rgb").unwrap(), 8, BitOrder::Msb, false, 10_000)
                .unwrap();
        assert_eq!(got.bytes.len(), 192);
        assert_eq!(got.bits_per_channel, 8);
    }

    #[test]
    fn bit_order_parse() {
        assert_eq!(BitOrder::parse("msb").unwrap(), BitOrder::Msb);
        assert_eq!(BitOrder::parse("LSB").unwrap(), BitOrder::Lsb);
        assert!(BitOrder::parse("foo").is_err());
    }

    #[test]
    fn sniff_filetype_signatures() {
        assert_eq!(sniff_filetype(b"\x89PNG\r\n\x1a\n....."), Some("PNG image"));
        assert_eq!(sniff_filetype(b"%PDF-1.7"), Some("PDF document"));
        assert_eq!(sniff_filetype(b"PK\x03\x04rest"), Some("ZIP archive (or docx/xlsx/jar)"));
        assert_eq!(sniff_filetype(b"hello text"), None);
        assert_eq!(sniff_filetype(b""), None);
    }

    #[test]
    fn channels_parse_variants() {
        assert_eq!(Channels::parse("rgb").unwrap().count(), 3);
        assert_eq!(Channels::parse("RGBA").unwrap().count(), 4);
        assert_eq!(Channels::parse("r").unwrap().count(), 1);
        assert_eq!(Channels::parse("gr").unwrap(), Channels::parse("rg").unwrap());
        assert_eq!(Channels::parse("rrg").unwrap(), Channels::parse("rg").unwrap());
        assert_eq!(Channels::parse("rgb").unwrap().label(), "rgb");
        assert_eq!(Channels::parse("ba").unwrap().label(), "ba");
        assert!(Channels::parse("").is_err());
        assert!(Channels::parse("xyz").is_err());
    }

    #[test]
    fn bad_bpc_errors() {
        let carrier = test_carrier(8, 8);
        let ch = Channels::parse("rgb").unwrap();
        assert!(extract_raw(&carrier, ch, 0, BitOrder::Msb, false, 100).is_err());
        assert!(extract_raw(&carrier, ch, 9, BitOrder::Msb, false, 100).is_err());
    }

    #[test]
    fn empty_input_errors() {
        assert!(extract_auto(&[]).is_err());
        assert!(
            extract_raw(&[], Channels::parse("rgb").unwrap(), 1, BitOrder::Msb, false, 100).is_err()
        );
    }
}
