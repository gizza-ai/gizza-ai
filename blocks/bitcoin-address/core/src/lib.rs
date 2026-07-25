//! bitcoin-address core — derive Bitcoin addresses (P2PKH, P2SH-P2WPKH, P2WPKH)
//! and WIF from a secp256k1 private key. Shared by the chat block, CLI, and web
//! page. Pure-Rust k256 (secp256k1) + SHA-256 + RIPEMD-160 + Base58Check / Bech32,
//! so it runs on every backend (native, wasm32-wasip1, browser wasm).

use k256::elliptic_curve::sec1::ToEncodedPoint;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_GEN: [u32; 5] = [
    0x3b6a_57b2,
    0x2650_8e6d,
    0x1ea1_19fa,
    0x3d42_33dd,
    0x2a14_62b3,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "mainnet" | "main" | "bitcoin" => Ok(Self::Mainnet),
            "testnet" | "test" => Ok(Self::Testnet),
            other => Err(format!(
                "unknown network '{other}' (use 'mainnet' or 'testnet')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Testnet => "testnet",
        }
    }
    fn p2pkh_version(self) -> u8 {
        if self == Self::Mainnet {
            0x00
        } else {
            0x6f
        }
    }
    fn p2sh_version(self) -> u8 {
        if self == Self::Mainnet {
            0x05
        } else {
            0xc4
        }
    }
    fn bech32_hrp(self) -> &'static str {
        if self == Self::Mainnet {
            "bc"
        } else {
            "tb"
        }
    }
    fn wif_version(self) -> u8 {
        if self == Self::Mainnet {
            0x80
        } else {
            0xef
        }
    }
    fn from_wif_version(v: u8) -> Option<Self> {
        match v {
            0x80 => Some(Self::Mainnet),
            0xef => Some(Self::Testnet),
            _ => None,
        }
    }
}

/// Derive the addresses + WIF for a secp256k1 private key.
///
/// * `key` — the private key as **hex** (64 chars, `0x` optional) or **WIF**
///   (base58check, `5…`/`K…`/`L…` mainnet, `9…`/`c…` testnet). WIF input carries
///   its own network + compression flag, which override the `network` /
///   `compressed` arguments.
/// * `network` — `"mainnet"` or `"testnet"` (only used for a hex key).
/// * `compressed` — use the compressed (33-byte) public key for P2PKH + WIF
///   (only used for a hex key). SegWit addresses always require a compressed key.
pub fn derive(key: &str, network: &str, compressed: bool) -> Result<String, String> {
    let net_arg = Network::parse(network)?;
    let (secret, network, compressed, source) = parse_key(key, net_arg, compressed)?;

    // secp256k1 public key in both encodings.
    let sk = k256::SecretKey::from_slice(&secret)
        .map_err(|_| "private key is not a valid secp256k1 scalar (must be non-zero and below the curve order)".to_string())?;
    let pk = sk.public_key();
    let pk_compressed = pk.to_encoded_point(true); // 33 bytes: 02/03 ‖ X
    let pk_uncompressed = pk.to_encoded_point(false); // 65 bytes: 04 ‖ X ‖ Y

    // The public key that P2PKH / the reported public_key_hex use.
    let pubkey_used = if compressed {
        pk_compressed.as_bytes()
    } else {
        pk_uncompressed.as_bytes()
    };

    let p2pkh = base58check(network.p2pkh_version(), &hash160(pubkey_used));

    // SegWit (BIP141/143) requires a compressed public key; an uncompressed key
    // yields non-standard, unspendable witness programs, so we don't emit one.
    let h160_compressed = hash160(pk_compressed.as_bytes());
    let (p2sh_p2wpkh, p2wpkh) = if compressed {
        let mut redeem = Vec::with_capacity(22);
        redeem.push(0x00); // witness version 0
        redeem.push(0x14); // push 20 bytes
        redeem.extend_from_slice(&h160_compressed);
        let p2sh = base58check(network.p2sh_version(), &hash160(&redeem));
        let bech = bech32_segwit_v0(network.bech32_hrp(), &h160_compressed);
        (p2sh, bech)
    } else {
        let note = "(requires a compressed key — set compressed=true)".to_string();
        (note.clone(), note)
    };

    let wif = wif(network, &secret, compressed);

    Ok(format!(
        "network: {}\ncompressed: {}\nkey_source: {}\nprivate_key_hex: {}\nprivate_key_wif: {}\npublic_key_hex: {}\npublic_key_hash160: {}\np2pkh: {}\np2sh_p2wpkh: {}\np2wpkh: {}",
        network.label(),
        compressed,
        source,
        hex::encode(secret),
        wif,
        hex::encode(pubkey_used),
        hex::encode(hash160(pubkey_used)),
        p2pkh,
        p2sh_p2wpkh,
        p2wpkh,
    ))
}

/// Returns (32-byte secret, network, compressed, source-label). Auto-detects hex
/// vs WIF; a WIF key's embedded network + compression flag win over the args.
fn parse_key(
    key: &str,
    net_arg: Network,
    compressed_arg: bool,
) -> Result<([u8; 32], Network, bool, &'static str), String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return Err("private key is required (hex or WIF)".into());
    }
    let cleaned: String = trimmed
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_')
        .collect();

    let looks_hex = !cleaned.is_empty() && cleaned.chars().all(|c| c.is_ascii_hexdigit());
    if looks_hex {
        if cleaned.len() != 64 {
            return Err(format!(
                "hex private key must be 32 bytes (64 hex chars), got {} chars",
                cleaned.len()
            ));
        }
        let bytes = hex::decode(&cleaned).map_err(|e| format!("invalid hex private key: {e}"))?;
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&bytes);
        return Ok((secret, net_arg, compressed_arg, "hex"));
    }

    // Otherwise treat it as WIF (base58check).
    let payload = bs58::decode(trimmed)
        .with_check(None)
        .into_vec()
        .map_err(|_| "key is neither valid hex nor a valid WIF (base58check failed)".to_string())?;
    if payload.is_empty() {
        return Err("WIF key is empty after decoding".into());
    }
    let network = Network::from_wif_version(payload[0]).ok_or_else(|| {
        format!(
            "unknown WIF version byte 0x{:02x} (expected 0x80 mainnet or 0xef testnet)",
            payload[0]
        )
    })?;
    let (secret_bytes, compressed) = match payload.len() {
        33 => (&payload[1..33], false), // version ‖ 32-byte key
        34 if payload[33] == 0x01 => (&payload[1..33], true), // ‖ 0x01 compressed flag
        34 => {
            return Err(format!(
                "invalid WIF compression flag 0x{:02x} (expected 0x01)",
                payload[33]
            ))
        }
        n => {
            return Err(format!(
                "WIF payload has unexpected length {n} (expected 33 or 34 bytes)"
            ))
        }
    };
    let mut secret = [0u8; 32];
    secret.copy_from_slice(secret_bytes);
    Ok((secret, network, compressed, "wif"))
}

fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let ripe = Ripemd160::digest(sha);
    ripe.as_slice().try_into().expect("RIPEMD-160 is 20 bytes")
}

fn base58check(version: u8, payload: &[u8]) -> String {
    let mut v = Vec::with_capacity(payload.len() + 1);
    v.push(version);
    v.extend_from_slice(payload);
    bs58::encode(v).with_check().into_string()
}

fn wif(network: Network, secret: &[u8; 32], compressed: bool) -> String {
    let mut v = Vec::with_capacity(34);
    v.push(network.wif_version());
    v.extend_from_slice(secret);
    if compressed {
        v.push(0x01);
    }
    bs58::encode(v).with_check().into_string()
}

fn bech32_segwit_v0(hrp: &str, program: &[u8; 20]) -> String {
    let mut data = Vec::with_capacity(1 + 32);
    data.push(0); // witness version 0
    data.extend(convert_bits(program, 8, 5, true));
    let checksum = bech32_checksum(hrp.as_bytes(), &data);
    let mut out = String::with_capacity(hrp.len() + 1 + data.len() + 6);
    out.push_str(hrp);
    out.push('1');
    for v in data.iter().chain(checksum.iter()) {
        out.push(BECH32_CHARSET[*v as usize] as char);
    }
    out
}

fn bech32_polymod(values: &[u8]) -> u32 {
    let mut chk = 1u32;
    for &v in values {
        let top = (chk >> 25) as u8;
        chk = ((chk & 0x1ff_ffff) << 5) ^ (v as u32);
        for (i, g) in BECH32_GEN.iter().enumerate() {
            if ((top >> i) & 1) != 0 {
                chk ^= g;
            }
        }
    }
    chk
}

fn bech32_checksum(hrp: &[u8], data: &[u8]) -> [u8; 6] {
    let mut values = Vec::with_capacity(hrp.len() * 2 + 1 + data.len() + 6);
    values.extend(hrp.iter().map(|b| b >> 5));
    values.push(0);
    values.extend(hrp.iter().map(|b| b & 31));
    values.extend_from_slice(data);
    values.extend_from_slice(&[0; 6]);
    let pm = bech32_polymod(&values) ^ 1;
    let mut out = [0u8; 6];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = ((pm >> (5 * (5 - i))) & 31) as u8;
    }
    out
}

fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Vec<u8> {
    let mut acc = 0u32;
    let mut bits = 0u32;
    let maxv = (1 << to) - 1;
    let mut ret = Vec::new();
    for value in data {
        acc = (acc << from) | (*value as u32);
        bits += from;
        while bits >= to {
            bits -= to;
            ret.push(((acc >> bits) & maxv) as u8);
        }
    }
    if pad && bits > 0 {
        ret.push(((acc << (to - bits)) & maxv) as u8);
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    // Well-known test vector: private key = 1.
    const K1: &str = "0000000000000000000000000000000000000000000000000000000000000001";

    fn field<'a>(out: &'a str, name: &str) -> &'a str {
        out.lines()
            .find_map(|l| l.strip_prefix(&format!("{name}: ")))
            .unwrap_or_else(|| panic!("missing field {name} in:\n{out}"))
    }

    #[test]
    fn compressed_mainnet_vectors() {
        let out = derive(K1, "mainnet", true).unwrap();
        assert_eq!(field(&out, "network"), "mainnet");
        assert_eq!(field(&out, "compressed"), "true");
        assert_eq!(field(&out, "key_source"), "hex");
        assert_eq!(field(&out, "private_key_hex"), K1);
        // Canonical compressed public key of privkey=1 (the generator point).
        assert_eq!(
            field(&out, "public_key_hex"),
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
        );
        // Well-known compressed P2PKH + WIF for privkey=1.
        assert_eq!(field(&out, "p2pkh"), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
        assert_eq!(
            field(&out, "private_key_wif"),
            "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn"
        );
        // Native SegWit is a valid bc1q… address.
        let bech = field(&out, "p2wpkh");
        assert!(bech.starts_with("bc1q") && bech.len() == 42, "{bech}");
        assert!(field(&out, "p2sh_p2wpkh").starts_with('3'));
    }

    #[test]
    fn uncompressed_mainnet_vectors() {
        let out = derive(K1, "mainnet", false).unwrap();
        assert_eq!(field(&out, "compressed"), "false");
        // Uncompressed pubkey begins with 0x04 then the generator X coord.
        assert!(field(&out, "public_key_hex")
            .starts_with("0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"));
        // Well-known uncompressed P2PKH + WIF for privkey=1.
        assert_eq!(field(&out, "p2pkh"), "1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm");
        assert_eq!(
            field(&out, "private_key_wif"),
            "5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf"
        );
        // No SegWit for an uncompressed key.
        assert!(field(&out, "p2wpkh").contains("requires a compressed key"));
        assert!(field(&out, "p2sh_p2wpkh").contains("requires a compressed key"));
    }

    #[test]
    fn testnet_prefixes() {
        let out = derive(K1, "testnet", true).unwrap();
        assert_eq!(field(&out, "network"), "testnet");
        assert!(field(&out, "p2pkh").starts_with('m') || field(&out, "p2pkh").starts_with('n'));
        assert!(field(&out, "p2wpkh").starts_with("tb1q"));
        assert!(field(&out, "p2sh_p2wpkh").starts_with('2'));
        assert!(field(&out, "private_key_wif").starts_with('c')); // testnet compressed WIF
    }

    #[test]
    fn accepts_0x_prefix_and_whitespace() {
        let out = derive(
            "0x00000000 00000000000000000000000000000000000000000000000000000001",
            "mainnet",
            true,
        )
        .unwrap();
        assert_eq!(field(&out, "p2pkh"), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
    }

    #[test]
    fn wif_input_roundtrips_network_and_compression() {
        // Compressed mainnet WIF for privkey=1 — network + compression come from the WIF.
        let out = derive(
            "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn",
            "testnet",
            false,
        )
        .unwrap();
        assert_eq!(field(&out, "key_source"), "wif");
        assert_eq!(field(&out, "network"), "mainnet"); // WIF version overrides the arg
        assert_eq!(field(&out, "compressed"), "true"); // WIF flag overrides the arg
        assert_eq!(field(&out, "private_key_hex"), K1);
        assert_eq!(field(&out, "p2pkh"), "1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH");
    }

    #[test]
    fn wif_uncompressed_input() {
        let out = derive(
            "5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAnchuDf",
            "mainnet",
            true,
        )
        .unwrap();
        assert_eq!(field(&out, "compressed"), "false");
        assert_eq!(field(&out, "p2pkh"), "1EHNa6Q4Jz2uvNExL497mE43ikXhwF6kZm");
    }

    #[test]
    fn rejects_empty_key() {
        assert!(derive("   ", "mainnet", true)
            .unwrap_err()
            .contains("required"));
    }

    #[test]
    fn rejects_wrong_hex_length() {
        let e = derive("0001", "mainnet", true).unwrap_err();
        assert!(e.contains("32 bytes"), "{e}");
    }

    #[test]
    fn rejects_zero_key() {
        let e = derive(
            "0000000000000000000000000000000000000000000000000000000000000000",
            "mainnet",
            true,
        )
        .unwrap_err();
        assert!(e.contains("valid secp256k1 scalar"), "{e}");
    }

    #[test]
    fn rejects_garbage() {
        let e = derive("not-a-real-key!!", "mainnet", true).unwrap_err();
        assert!(e.contains("neither valid hex nor a valid WIF"), "{e}");
    }

    #[test]
    fn rejects_bad_network() {
        assert!(derive(K1, "dogecoin", true)
            .unwrap_err()
            .contains("unknown network"));
    }
}
