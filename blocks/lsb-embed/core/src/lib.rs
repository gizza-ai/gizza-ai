//! lsb-embed core — pure-Rust LSB (least-significant-bit) steganography.
//!
//! Hides a UTF-8 text payload inside the least-significant bits of an image's
//! pixel channels and re-encodes the result as a lossless PNG (the carrier MUST
//! stay lossless — a JPEG re-encode would destroy the hidden bits). A matching
//! [`extract`] walks the same bit order and reconstructs the payload (kept here
//! for unit-test round-tripping and reuse by a future lsb-extract tool).
//!
//! Wire format written into the LSBs (bit-by-bit, MSB-first within each byte):
//!   [MAGIC "LSB1" (4 bytes)] [bits_per_channel (1 byte)] [length N (4 bytes, BE)] [N payload bytes]
//! so the extractor knows the depth and exactly how many bytes to read. We embed
//! across the R, G and B channels (skipping alpha so a fully-transparent pixel
//! can't silently drop data) at a configurable number of bits/channel (1..=4),
//! trading capacity for visibility.
//!
//! Pure (no wafer/wasm deps): unit-testable on the host, reused by the chat
//! skill block. Image-bytes output → chat + CLI only, no page.

use image::{ImageFormat, RgbaImage};
use std::io::Cursor;

const MAGIC: &[u8; 4] = b"LSB1";
const HEADER_LEN: usize = 9; // MAGIC(4) + bpc(1) + len(4)

/// Embed `payload` bytes into `image_bytes`, returning a PNG.
///
/// `bits_per_channel` (1..=4) trades capacity for visibility — 1 is least
/// detectable, 4 hides four times as much. Errors if the payload doesn't fit.
pub fn embed(image_bytes: &[u8], payload: &[u8], bits_per_channel: u8) -> Result<Vec<u8>, String> {
    let bpc = validate_bpc(bits_per_channel)?;
    let img = image::load_from_memory(image_bytes).map_err(|e| format!("decode image: {e}"))?;
    let mut rgba: RgbaImage = img.to_rgba8();

    let mut stream = Vec::with_capacity(HEADER_LEN + payload.len());
    stream.extend_from_slice(MAGIC);
    stream.push(bpc);
    stream.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    stream.extend_from_slice(payload);

    let total_bits = stream.len() * 8;
    let capacity_bits = rgba.pixels().len() * 3 * bpc as usize;
    if total_bits > capacity_bits {
        let max_payload = capacity_bits / 8 - HEADER_LEN.min(capacity_bits / 8);
        return Err(format!(
            "payload too large: needs {} bytes but this image holds about {} at {} bit(s)/channel — use a bigger image or raise bits_per_channel",
            payload.len(),
            max_payload,
            bpc
        ));
    }

    let mask = !((1u8 << bpc) - 1); // clear the low `bpc` bits of a channel
    let mut reader = BitReader::new(&stream);
    'outer: for px in rgba.pixels_mut() {
        for ch in 0..3 {
            // Fill the `bpc`-bit field MSB-first (matching the decoder's
            // `shift = bpc-1-sub` read order). A partial final group leaves the
            // low positions zero, so a bit goes to its correct high slot.
            let mut val = 0u8;
            let mut got = 0u8;
            for _ in 0..bpc {
                match reader.next() {
                    Some(b) => {
                        val = (val << 1) | b;
                        got += 1;
                    }
                    None => break,
                }
            }
            if got == 0 {
                break 'outer; // nothing more to write
            }
            // Left-align a partial group into the high `bpc` bits.
            val <<= bpc - got;
            px.0[ch] = (px.0[ch] & mask) | val;
            if got < bpc {
                break 'outer;
            }
        }
    }

    let mut out = Cursor::new(Vec::new());
    rgba
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| format!("encode PNG: {e}"))?;
    Ok(out.into_inner())
}

/// Extract a payload previously embedded by [`embed`]. Returns the raw bytes.
/// Errors if no valid lsb-embed header is present.
pub fn extract(image_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(image_bytes).map_err(|e| format!("decode image: {e}"))?;
    let rgba: RgbaImage = img.to_rgba8();
    for bpc in 1u8..=4 {
        if let Some(payload) = try_extract_at(&rgba, bpc) {
            return Ok(payload);
        }
    }
    Err("no lsb-embed payload found in image (wrong image, or it was re-encoded lossily)".into())
}

fn try_extract_at(rgba: &RgbaImage, bpc: u8) -> Option<Vec<u8>> {
    let mut bits = BitCollector::new(rgba, bpc);
    let header = bits.read_bytes(HEADER_LEN)?;
    if &header[0..4] != MAGIC || header[4] != bpc {
        return None;
    }
    let len = u32::from_be_bytes([header[5], header[6], header[7], header[8]]) as usize;
    bits.read_bytes(len)
}

fn validate_bpc(bpc: u8) -> Result<u8, String> {
    if (1..=4).contains(&bpc) {
        Ok(bpc)
    } else {
        Err(format!("bits_per_channel must be 1..=4, got {bpc}"))
    }
}

/// Reads a byte slice bit-by-bit, MSB-first.
struct BitReader<'a> {
    bytes: &'a [u8],
    bit: usize,
}
impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }
    fn next(&mut self) -> Option<u8> {
        let byte_idx = self.bit / 8;
        if byte_idx >= self.bytes.len() {
            return None;
        }
        let shift = 7 - (self.bit % 8);
        let b = (self.bytes[byte_idx] >> shift) & 1;
        self.bit += 1;
        Some(b)
    }
}

/// Walks pixel channels (R,G,B; alpha skipped) pulling `bpc` low bits from each,
/// MSB-first, reassembling the embedded bit stream back into bytes.
struct BitCollector<'a> {
    raw: &'a [u8], // RGBA8, 4 bytes per pixel
    bpc: u8,
    px: usize,
    ch: usize,
    sub: u8,
    acc: u8,
    acc_bits: u8,
}
impl<'a> BitCollector<'a> {
    fn new(rgba: &'a RgbaImage, bpc: u8) -> Self {
        Self { raw: rgba.as_raw(), bpc, px: 0, ch: 0, sub: 0, acc: 0, acc_bits: 0 }
    }
    fn next_bit(&mut self) -> Option<u8> {
        loop {
            let base = self.px * 4;
            if base >= self.raw.len() {
                return None;
            }
            if self.ch > 2 {
                self.px += 1;
                self.ch = 0;
                self.sub = 0;
                continue;
            }
            let channel_val = self.raw[base + self.ch];
            let shift = self.bpc - 1 - self.sub;
            let b = (channel_val >> shift) & 1;
            self.sub += 1;
            if self.sub >= self.bpc {
                self.sub = 0;
                self.ch += 1;
            }
            return Some(b);
        }
    }
    fn read_bytes(&mut self, n: usize) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            for _ in 0..8 {
                let bit = self.next_bit()?;
                self.acc = (self.acc << 1) | bit;
                self.acc_bits += 1;
                if self.acc_bits == 8 {
                    out.push(self.acc);
                    self.acc = 0;
                    self.acc_bits = 0;
                }
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn test_png(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbaImage::new(w, h);
        for (i, px) in img.pixels_mut().enumerate() {
            let v = (i % 200) as u8;
            *px = Rgba([v, 255 - v, v.wrapping_mul(3), 255]);
        }
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn round_trip_text() {
        let carrier = test_png(64, 64);
        let secret = "hello, hidden world! \u{1F511}".as_bytes(); // includes emoji bytes
        let stego = embed(&carrier, secret, 1).unwrap();
        let got = extract(&stego).unwrap();
        assert_eq!(got, secret);
    }

    #[test]
    fn round_trip_higher_depth() {
        let carrier = test_png(64, 64); // 1-bit capacity ~1.5 KB, fits 500-byte payload
        let secret = vec![0xABu8; 500];
        for bpc in 1..=4u8 {
            let stego = embed(&carrier, &secret, bpc).unwrap();
            let got = extract(&stego).unwrap();
            assert_eq!(got, secret, "bpc {bpc} round-trip");
        }
    }

    #[test]
    fn output_is_valid_png() {
        let carrier = test_png(16, 16);
        let stego = embed(&carrier, b"x", 1).unwrap();
        assert_eq!(image::guess_format(&stego).unwrap(), ImageFormat::Png);
    }

    #[test]
    fn too_large_payload_errors() {
        let carrier = test_png(8, 8); // 64 px * 3 ch * 1 bit = 192 bits = 24 bytes total
        let err = embed(&carrier, &vec![0u8; 1000], 1).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn bad_bpc_errors() {
        let carrier = test_png(8, 8);
        assert!(embed(&carrier, b"x", 0).is_err());
        assert!(embed(&carrier, b"x", 5).is_err());
    }

    #[test]
    fn extract_from_clean_image_errors() {
        let carrier = test_png(40, 40);
        assert!(extract(&carrier).is_err());
    }

    #[test]
    fn stego_is_visually_close_to_carrier() {
        let carrier = test_png(32, 32);
        let stego = embed(&carrier, b"some secret text here", 1).unwrap();
        let a = image::load_from_memory(&carrier).unwrap().to_rgba8();
        let b = image::load_from_memory(&stego).unwrap().to_rgba8();
        let max_delta = a
            .as_raw()
            .iter()
            .zip(b.as_raw())
            .map(|(x, y)| (*x as i16 - *y as i16).unsigned_abs())
            .max()
            .unwrap();
        assert!(max_delta <= 1, "1-bit LSB differs by at most 1, got {max_delta}");
    }

    #[test]
    fn empty_payload_round_trips() {
        let carrier = test_png(16, 16);
        let stego = embed(&carrier, b"", 1).unwrap();
        assert_eq!(extract(&stego).unwrap(), Vec::<u8>::new());
    }
}
