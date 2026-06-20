//! gizza-ai/file-hash core — pure-Rust checksums of arbitrary bytes. No
//! wafer/wasm-bindgen deps. Computes MD5, SHA-1, SHA-256, SHA-512, and CRC-32 in
//! a single pass-equivalent and returns lowercase hex digests. All hashers are
//! RustCrypto (pure-Rust, wasm32-safe).

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

/// All digests for one input, as lowercase hex strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
    /// CRC-32 (IEEE, as used by zip/gzip), 8 lowercase hex digits.
    pub crc32: String,
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// CRC-32 (IEEE 802.3, reflected) — the variant used by zip, gzip, PNG.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Compute every supported digest for `data`.
pub fn hash_all(data: &[u8]) -> Hashes {
    Hashes {
        md5: hex(&Md5::digest(data)),
        sha1: hex(&Sha1::digest(data)),
        sha256: hex(&Sha256::digest(data)),
        sha512: hex(&Sha512::digest(data)),
        crc32: format!("{:08x}", crc32(data)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_known_vectors() {
        let h = hash_all(b"");
        assert_eq!(h.md5, "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(h.sha1, "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            h.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            h.sha512,
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
        assert_eq!(h.crc32, "00000000");
    }

    #[test]
    fn abc_known_vectors() {
        let h = hash_all(b"abc");
        assert_eq!(h.md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(h.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            h.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // CRC-32 of "abc" is 0x352441c2.
        assert_eq!(h.crc32, "352441c2");
    }

    #[test]
    fn hex_is_lowercase_and_padded() {
        assert_eq!(hex(&[0x00, 0x0f, 0xff]), "000fff");
    }

    #[test]
    fn digest_lengths() {
        let h = hash_all(b"some bytes");
        assert_eq!(h.md5.len(), 32);
        assert_eq!(h.sha1.len(), 40);
        assert_eq!(h.sha256.len(), 64);
        assert_eq!(h.sha512.len(), 128);
        assert_eq!(h.crc32.len(), 8);
    }
}
