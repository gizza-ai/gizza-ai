//! hd-key-derive core — BIP32 private HD key derivation for Bitcoin mainnet/testnet.
//! Shared by the chat block, CLI, and web page. Uses pure-Rust k256 via the bip32 crate.

use bip32::{DerivationPath, Prefix, PublicKey, XPrv};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use std::str::FromStr;

const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_GEN: [u32; 5] = [0x3b6a_57b2, 0x2650_8e6d, 0x1ea1_19fa, 0x3d42_33dd, 0x2a14_62b3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "mainnet" | "main" | "bitcoin" => Ok(Self::Mainnet),
            "testnet" | "test" => Ok(Self::Testnet),
            other => Err(format!("unknown network '{other}' (use 'mainnet' or 'testnet')")),
        }
    }
    fn label(self) -> &'static str {
        match self { Self::Mainnet => "mainnet", Self::Testnet => "testnet" }
    }
    fn xprv_prefix(self) -> Prefix {
        match self { Self::Mainnet => Prefix::XPRV, Self::Testnet => Prefix::TPRV }
    }
    fn xpub_prefix(self) -> Prefix {
        match self { Self::Mainnet => Prefix::XPUB, Self::Testnet => Prefix::TPUB }
    }
    fn p2pkh_version(self) -> u8 { if self == Self::Mainnet { 0x00 } else { 0x6f } }
    fn p2sh_version(self) -> u8 { if self == Self::Mainnet { 0x05 } else { 0xc4 } }
    fn bech32_hrp(self) -> &'static str { if self == Self::Mainnet { "bc" } else { "tb" } }
    fn wif_version(self) -> u8 { if self == Self::Mainnet { 0x80 } else { 0xef } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressType {
    P2pkh,
    P2shP2wpkh,
    P2wpkh,
}

impl AddressType {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "" | "p2pkh" | "legacy" => Ok(Self::P2pkh),
            "p2sh_p2wpkh" | "wrapped" | "nested_segwit" => Ok(Self::P2shP2wpkh),
            "p2wpkh" | "bech32" | "native_segwit" => Ok(Self::P2wpkh),
            other => Err(format!(
                "unknown address_type '{other}' (use 'p2pkh', 'p2sh_p2wpkh', or 'p2wpkh')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self { Self::P2pkh => "p2pkh", Self::P2shP2wpkh => "p2sh_p2wpkh", Self::P2wpkh => "p2wpkh" }
    }
}

/// Derive an HD private key from a hex seed or an xprv.
/// Exactly one of `seed_hex` or `xprv` must be non-empty. `path` accepts BIP32
/// paths such as `m`, `m/0'`, `m/44'/0'/0'/0/0`; hardened suffixes may be `'`,
/// `h`, or `H`.
pub fn derive(
    seed_hex: &str,
    xprv: &str,
    path: &str,
    network: &str,
    address_type: &str,
) -> Result<String, String> {
    let network = Network::parse(network)?;
    let address_type = AddressType::parse(address_type)?;
    let seed_hex = seed_hex.trim();
    let xprv = xprv.trim();
    if seed_hex.is_empty() == xprv.is_empty() {
        return Err("provide exactly one of seed or xprv".into());
    }

    let root = if !seed_hex.is_empty() {
        let seed = parse_hex(seed_hex)?;
        if ![16usize, 32, 64].contains(&seed.len()) {
            return Err(format!(
                "seed must be 16, 32, or 64 bytes for this BIP32 backend, got {} bytes",
                seed.len()
            ));
        }
        XPrv::new(&seed).map_err(|e| format!("invalid seed: {e}"))?
    } else {
        let lower = xprv.to_ascii_lowercase();
        if network == Network::Mainnet && !lower.starts_with("xprv") {
            return Err("mainnet expects an xprv key (testnet uses tprv)".into());
        }
        if network == Network::Testnet && !lower.starts_with("tprv") {
            return Err("testnet expects a tprv key (mainnet uses xprv)".into());
        }
        XPrv::from_str(xprv).map_err(|e| format!("invalid xprv/tprv: {e}"))?
    };

    let path = normalize_path(path)?;
    let derivation_path = DerivationPath::from_str(&path).map_err(|e| format!("invalid path: {e}"))?;
    let child = derivation_path
        .iter()
        .fold(Ok(root), |key, child_num| key.and_then(|k| k.derive_child(child_num)))
        .map_err(|e| format!("derivation failed: {e}"))?;
    let xpub = child.public_key();
    let private_key: [u8; 32] = child.private_key().to_bytes().into();
    let public_key = xpub.public_key().to_bytes();
    let hash160 = hash160(&public_key);
    let address = match address_type {
        AddressType::P2pkh => base58check(network.p2pkh_version(), &hash160),
        AddressType::P2shP2wpkh => {
            let mut redeem = Vec::with_capacity(22);
            redeem.push(0x00);
            redeem.push(0x14);
            redeem.extend_from_slice(&hash160);
            base58check(network.p2sh_version(), &hash160_bytes(&redeem))
        }
        AddressType::P2wpkh => bech32_segwit_v0(network.bech32_hrp(), &hash160),
    };
    let fingerprint = &hash160[..4];

    let xprv_s = child.to_string(network.xprv_prefix());
    let xpub_s = xpub.to_string(network.xpub_prefix());

    Ok(format!(
        "path: {path}\nnetwork: {}\naddress_type: {}\ndepth: {}\nparent_fingerprint: {}\nfingerprint: {}\nxprv: {}\nxpub: {}\nprivate_key_hex: {}\nprivate_key_wif: {}\npublic_key_compressed_hex: {}\naddress: {}",
        network.label(),
        address_type.label(),
        child.attrs().depth,
        hex::encode(child.attrs().parent_fingerprint),
        hex::encode(fingerprint),
        xprv_s.as_str(),
        xpub_s.as_str(),
        hex::encode(private_key),
        wif(network, &private_key),
        hex::encode(public_key),
        address
    ))
}

fn parse_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X")
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != '_' && *c != '-')
        .collect();
    if cleaned.is_empty() { return Err("seed is empty".into()); }
    if cleaned.len() % 2 != 0 { return Err(format!("seed hex has odd length {}", cleaned.len())); }
    hex::decode(&cleaned).map_err(|e| format!("invalid seed hex: {e}"))
}

fn normalize_path(path: &str) -> Result<String, String> {
    let p = path.trim();
    if p.is_empty() { return Err("path is required, e.g. m/44'/0'/0'/0/0".into()); }
    if p == "m" || p == "M" { return Ok("m".into()); }
    let rest = p
        .strip_prefix("m/")
        .or_else(|| p.strip_prefix("M/"))
        .ok_or("path must start with m or m/ (for example m/44'/0'/0'/0/0)")?;
    let mut out = String::from("m");
    for raw in rest.split('/') {
        if raw.is_empty() { return Err("path contains an empty segment".into()); }
        let (digits, hardened) = match raw.as_bytes().last().copied() {
            Some(b'\'') => (&raw[..raw.len() - 1], true),
            Some(b'h') | Some(b'H') => (&raw[..raw.len() - 1], true),
            _ => (raw, false),
        };
        let idx: u32 = digits.parse().map_err(|_| format!("invalid path segment '{raw}'"))?;
        if idx >= (1 << 31) {
            return Err(format!("path segment '{raw}' is too large; use values below 2^31"));
        }
        out.push('/');
        out.push_str(&idx.to_string());
        if hardened { out.push('\''); }
    }
    Ok(out)
}

fn hash160(data: &[u8]) -> [u8; 20] { hash160_bytes(data) }

fn hash160_bytes(data: &[u8]) -> [u8; 20] {
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

fn wif(network: Network, private_key: &[u8; 32]) -> String {
    let mut v = Vec::with_capacity(34);
    v.push(network.wif_version());
    v.extend_from_slice(private_key);
    v.push(0x01); // compressed public key
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
            if ((top >> i) & 1) != 0 { chk ^= g; }
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
    if pad && bits > 0 { ret.push(((acc << (to - bits)) & maxv) as u8); }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    const TV1_SEED: &str = "000102030405060708090a0b0c0d0e0f";

    #[test]
    fn derives_bip32_vector_master() {
        let out = derive(TV1_SEED, "", "m", "mainnet", "p2pkh").unwrap();
        assert!(out.contains("xprv: xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi"), "{out}");
        assert!(out.contains("xpub: xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFjqJoCu1Rupje8YtGqsefD265TMg7usUDFdp6W1EGMcet8"), "{out}");
        assert!(out.contains("depth: 0"));
        assert!(out.contains("address: 15mKKb2eos1hWa6tisdPwwDC1a5J1y9nma"));
    }

    #[test]
    fn derives_bip32_vector_child_h_suffix() {
        let out = derive(TV1_SEED, "", "m/0h", "mainnet", "p2pkh").unwrap();
        assert!(out.contains("path: m/0'"));
        assert!(out.contains("xprv: xprv9uHRZZhk6KAJC1avXpDAp4MDc3sQKNxDiPvvkX8Br5ngLNv1TxvUxt4cV1rGL5hj6KCesnDYUhd7oWgT11eZG7XnxHrnYeSvkzY7d2bhkJ7"), "{out}");
        assert!(out.contains("xpub: xpub68Gmy5EdvgibQVfPdqkBBCHxA5htiqg55crXYuXoQRKfDBFA1WEjWgP6LHhwBZeNK1VTsfTFUHCdrfp1bgwQ9xv5ski8PX9rL2dZXvgGDnw"), "{out}");
        assert!(out.contains("address: 19Q2WoS5hSS6T8GjhK8KZLMgmWaq4neXrh"));
    }

    #[test]
    fn derives_native_segwit_testnet_address() {
        let out = derive(TV1_SEED, "", "m/84'/1'/0'/0/0", "testnet", "p2wpkh").unwrap();
        assert!(out.contains("network: testnet"));
        assert!(out.contains("address_type: p2wpkh"));
        assert!(out.contains("xprv: tprv"), "{out}");
        assert!(out.contains("address: tb1"), "{out}");
    }

    #[test]
    fn derives_from_xprv() {
        let root = derive(TV1_SEED, "", "m", "mainnet", "p2pkh").unwrap();
        let xprv = root.lines().find_map(|l| l.strip_prefix("xprv: ")).unwrap();
        let child = derive("", xprv, "m/0'", "mainnet", "p2sh_p2wpkh").unwrap();
        assert!(child.contains("address_type: p2sh_p2wpkh"));
        assert!(child.contains("address: 3"), "{child}");
    }

    #[test]
    fn rejects_missing_key_material() {
        let e = derive("", "", "m/0", "mainnet", "p2pkh").unwrap_err();
        assert!(e.contains("exactly one"));
    }

    #[test]
    fn rejects_bad_path() {
        let e = derive(TV1_SEED, "", "44'/0'", "mainnet", "p2pkh").unwrap_err();
        assert!(e.contains("must start with m"));
    }
}
