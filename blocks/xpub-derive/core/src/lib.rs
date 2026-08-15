//! xpub-derive core — watch-only address derivation from an extended PUBLIC key.
//!
//! Takes an account-level extended public key (`xpub`/`ypub`/`zpub` on mainnet,
//! `tpub`/`upub`/`vpub` on testnet) and walks the BIP32 non-hardened children
//! `m/0/i` (receive) and `m/1/i` (change), rendering a Bitcoin address for each.
//! Pure public-key derivation — no private key is ever accepted or produced.
//! Shared by the chat block, the CLI, and the web page.

use bip32::{ChildNumber, ExtendedKey, XPub};
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use std::str::FromStr;

const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_GEN: [u32; 5] = [
    0x3b6a_57b2,
    0x2650_8e6d,
    0x1ea1_19fa,
    0x3d42_33dd,
    0x2a14_62b3,
];

/// Highest non-hardened BIP32 child index (2^31 - 1).
pub const MAX_INDEX: u32 = (1 << 31) - 1;
/// Largest number of addresses that may be requested per chain.
pub const MAX_COUNT: u32 = 100;
/// Default number of addresses per chain when the caller does not say.
pub const DEFAULT_COUNT: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Network {
    Mainnet,
    Testnet,
}

impl Network {
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressType {
    P2pkh,
    P2shP2wpkh,
    P2wpkh,
}

impl AddressType {
    fn label(self) -> &'static str {
        match self {
            Self::P2pkh => "p2pkh",
            Self::P2shP2wpkh => "p2sh_p2wpkh",
            Self::P2wpkh => "p2wpkh",
        }
    }
}

/// `auto` keeps whatever the key's SLIP-132 prefix implies.
fn parse_address_type(s: &str) -> Result<Option<AddressType>, String> {
    match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "" | "auto" => Ok(None),
        "p2pkh" | "legacy" => Ok(Some(AddressType::P2pkh)),
        "p2sh_p2wpkh" | "wrapped" | "nested_segwit" => Ok(Some(AddressType::P2shP2wpkh)),
        "p2wpkh" | "bech32" | "native_segwit" => Ok(Some(AddressType::P2wpkh)),
        other => Err(format!(
            "unknown address_type '{other}' (use 'auto', 'p2pkh', 'p2sh_p2wpkh', or 'p2wpkh')"
        )),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chain {
    Receive,
    Change,
    Both,
}

impl Chain {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "all" => Ok(Self::Both),
            "receive" | "external" | "deposit" => Ok(Self::Receive),
            "change" | "internal" => Ok(Self::Change),
            other => Err(format!(
                "unknown chain '{other}' (use 'receive', 'change', or 'both')"
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Receive => "receive",
            Self::Change => "change",
            Self::Both => "both",
        }
    }
    /// (chain index, label) pairs in output order.
    fn branches(self) -> Vec<(u32, &'static str)> {
        match self {
            Self::Receive => vec![(0, "receive")],
            Self::Change => vec![(1, "change")],
            Self::Both => vec![(0, "receive"), (1, "change")],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Table,
    Csv,
    List,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" => Ok(Self::Table),
            "csv" => Ok(Self::Csv),
            "list" | "addresses" => Ok(Self::List),
            other => Err(format!(
                "unknown format '{other}' (use 'table', 'csv', or 'list')"
            )),
        }
    }
}

/// Map a SLIP-132 prefix to the network and the address type it implies.
fn classify_prefix(prefix: &str) -> Result<(Network, AddressType), String> {
    match prefix {
        "xpub" => Ok((Network::Mainnet, AddressType::P2pkh)),
        "ypub" => Ok((Network::Mainnet, AddressType::P2shP2wpkh)),
        "zpub" => Ok((Network::Mainnet, AddressType::P2wpkh)),
        "tpub" => Ok((Network::Testnet, AddressType::P2pkh)),
        "upub" => Ok((Network::Testnet, AddressType::P2shP2wpkh)),
        "vpub" => Ok((Network::Testnet, AddressType::P2wpkh)),
        "Ypub" | "Zpub" | "Upub" | "Vpub" => Err(format!(
            "'{prefix}' is a multi-signature extended key; this tool derives single-signature \
             addresses only (use xpub, ypub, zpub, tpub, upub, or vpub)"
        )),
        other if other.to_ascii_lowercase().ends_with("prv") => Err(format!(
            "'{other}' is an extended PRIVATE key — never paste one into a derivation tool. \
             Use the matching public key (xpub, ypub, zpub, tpub, upub, or vpub)"
        )),
        other => Err(format!(
            "unsupported extended-key prefix '{other}' (expected xpub, ypub, zpub, tpub, upub, or vpub)"
        )),
    }
}

/// String-in/string-out wrapper for the browser page, where every field value
/// arrives as a raw string. Blank values fall back to the documented defaults.
#[allow(clippy::too_many_arguments)]
pub fn derive_str(
    xpub: &str,
    chain: &str,
    count: &str,
    start: &str,
    address_type: &str,
    format: &str,
    include_public_key: &str,
) -> Result<String, String> {
    let count = parse_u32(count, "count", DEFAULT_COUNT)?;
    let start = parse_u32(start, "start", 0)?;
    let include_public_key = matches!(
        include_public_key.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    );
    derive(
        xpub,
        chain,
        count,
        start,
        address_type,
        format,
        include_public_key,
    )
}

fn parse_u32(raw: &str, field: &str, default: u32) -> Result<u32, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<u32>()
        .map_err(|_| format!("{field} must be a whole number, got '{t}'"))
}

/// Derive `count` addresses per selected chain from an extended public key.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    xpub: &str,
    chain: &str,
    count: u32,
    start: u32,
    address_type: &str,
    format: &str,
    include_public_key: bool,
) -> Result<String, String> {
    let chain = Chain::parse(chain)?;
    let format = Format::parse(format)?;
    let override_type = parse_address_type(address_type)?;

    if count == 0 || count > MAX_COUNT {
        return Err(format!(
            "count must be between 1 and {MAX_COUNT}, got {count}"
        ));
    }
    if start > MAX_INDEX {
        return Err(format!(
            "start must be a non-hardened index between 0 and {MAX_INDEX}, got {start}"
        ));
    }
    if start as u64 + count as u64 - 1 > MAX_INDEX as u64 {
        return Err(format!(
            "start {start} plus count {count} runs past the highest non-hardened index {MAX_INDEX}"
        ));
    }

    let trimmed = xpub.trim();
    if trimmed.is_empty() {
        return Err("xpub is required — paste an extended public key such as xpub6…, ypub6…, zpub6…, tpub…, upub…, or vpub…".into());
    }
    let extended = ExtendedKey::from_str(trimmed).map_err(|e| {
        format!("invalid extended key: {e} (expected a base58check xpub/ypub/zpub/tpub/upub/vpub)")
    })?;
    let prefix = extended.prefix.as_str().to_string();
    let (network, prefix_type) = classify_prefix(&prefix)?;
    let address_type = override_type.unwrap_or(prefix_type);

    let account = XPub::try_from(extended)
        .map_err(|e| format!("could not read the extended public key: {e}"))?;
    let account_depth = account.attrs().depth;
    let account_parent = hex::encode(account.attrs().parent_fingerprint);
    let account_fingerprint = hex::encode(account.fingerprint());

    let mut rows: Vec<(&'static str, u32, String, String, String)> = Vec::new();
    for (branch, label) in chain.branches() {
        let branch_key = account
            .derive_child(
                ChildNumber::new(branch, false).map_err(|e| format!("bad chain index: {e}"))?,
            )
            .map_err(|e| format!("could not derive the {label} chain (m/{branch}): {e}"))?;
        for index in start..start.saturating_add(count) {
            let child = branch_key
                .derive_child(
                    ChildNumber::new(index, false)
                        .map_err(|e| format!("bad index {index}: {e}"))?,
                )
                .map_err(|e| format!("could not derive m/{branch}/{index}: {e}"))?;
            let public_key = child.to_bytes();
            let address = encode_address(network, address_type, &hash160(&public_key));
            rows.push((
                label,
                index,
                format!("m/{branch}/{index}"),
                address,
                hex::encode(public_key),
            ));
        }
    }

    Ok(match format {
        Format::List => rows
            .iter()
            .map(|r| r.3.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Csv => {
            let mut out = String::from("chain,index,path,address");
            if include_public_key {
                out.push_str(",public_key");
            }
            for (label, index, path, address, public_key) in &rows {
                out.push_str(&format!("\n{label},{index},{path},{address}"));
                if include_public_key {
                    out.push(',');
                    out.push_str(public_key);
                }
            }
            out
        }
        Format::Table => {
            let last = start + count - 1;
            let mut out = format!(
                "network: {}\naddress_type: {}\nkey_prefix: {}\ndepth: {}\nparent_fingerprint: {}\nfingerprint: {}\nchain: {}\nindex_range: {}-{}\naddresses_per_chain: {}",
                network.label(),
                address_type.label(),
                prefix,
                account_depth,
                account_parent,
                account_fingerprint,
                chain.label(),
                start,
                last,
                count
            );
            let width = rows.iter().map(|r| r.2.len()).max().unwrap_or(0);
            for (branch, label) in chain.branches() {
                out.push_str(&format!("\n\n{label} addresses (m/{branch}/i):"));
                for (row_label, _, path, address, public_key) in
                    rows.iter().filter(|r| r.0 == label)
                {
                    let _ = row_label;
                    out.push_str(&format!("\n{path:<width$}  {address}"));
                    if include_public_key {
                        out.push_str("  ");
                        out.push_str(public_key);
                    }
                }
            }
            out
        }
    })
}

fn encode_address(network: Network, address_type: AddressType, hash160: &[u8; 20]) -> String {
    match address_type {
        AddressType::P2pkh => base58check(network.p2pkh_version(), hash160),
        AddressType::P2shP2wpkh => {
            let mut redeem = Vec::with_capacity(22);
            redeem.push(0x00);
            redeem.push(0x14);
            redeem.extend_from_slice(hash160);
            base58check(network.p2sh_version(), &hash160_bytes(&redeem))
        }
        AddressType::P2wpkh => bech32_segwit_v0(network.bech32_hrp(), hash160),
    }
}

fn hash160(data: &[u8]) -> [u8; 20] {
    hash160_bytes(data)
}

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

    /// BIP84 test vector, account 0 (m/84'/0'/0') of the "abandon … about" mnemonic.
    const ZPUB: &str = "zpub6rFR7y4Q2AijBEqTUquhVz398htDFrtymD9xYYfG1m4wAcvPhXNfE3EfH1r1ADqtfSdVCToUG868RvUUkgDKf31mGDtKsAYz2oz2AGutZYs";
    /// BIP49 test vector, testnet account 0 (m/49'/1'/0').
    const UPUB: &str = "upub5EFU65HtV5TeiSHmZZm7FUffBGy8UKeqp7vw43jYbvZPpoVsgU93oac7Wk3u6moKegAEWtGNF8DehrnHtv21XXEMYRUocHqguyjknFHYfgY";
    /// BIP32 test vector 1 master public key.
    const XPUB: &str = "xpub661MyMwAqRbcFtXgS5sYJABqqG9YLmC4Q1Rdap9gSE8NqtwybGhePY2gZ29ESFjqJoCu1Rupje8YtGqsefD265TMg7usUDFdp6W1EGMcet8";
    const XPRV: &str = "xprv9s21ZrQH143K3QTDL4LXw2F7HEK3wJUD2nW2nRk4stbPy6cq3jPPqjiChkVvvNKmPGJxWUtg6LnF5kejMRNNU3TGtRBeJgk33yuGBxrMPHi";

    #[test]
    fn bip84_zpub_receive_and_change_vectors() {
        let out = derive(ZPUB, "both", 2, 0, "auto", "table", false).unwrap();
        assert!(out.contains("network: mainnet"), "{out}");
        assert!(out.contains("address_type: p2wpkh"), "{out}");
        assert!(out.contains("key_prefix: zpub"), "{out}");
        assert!(out.contains("depth: 3"), "{out}");
        // BIP84 first + second receiving addresses and first change address.
        assert!(
            out.contains("m/0/0  bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"),
            "{out}"
        );
        assert!(
            out.contains("m/0/1  bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g"),
            "{out}"
        );
        assert!(
            out.contains("m/1/0  bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el"),
            "{out}"
        );
    }

    #[test]
    fn bip49_upub_derives_testnet_nested_segwit() {
        let out = derive(UPUB, "receive", 1, 0, "auto", "table", true).unwrap();
        assert!(out.contains("network: testnet"), "{out}");
        assert!(out.contains("address_type: p2sh_p2wpkh"), "{out}");
        // BIP49 test-vector address + public key for m/49'/1'/0'/0/0.
        assert!(out.contains("2Mww8dCYPUpKHofjgcXcBCEGmniw9CoaiD2"), "{out}");
        assert!(
            out.contains("03a1af804ac108a8a51782198c2d034b28bf90c8803f5a53f76276fa69a4eae77f"),
            "{out}"
        );
    }

    #[test]
    fn xpub_defaults_to_legacy_p2pkh() {
        let out = derive(XPUB, "receive", 1, 0, "auto", "list", false).unwrap();
        // Cross-checked against hd-key-derive's private-side m/0/0 for the same
        // BIP32 test-vector-1 seed (000102030405060708090a0b0c0d0e0f).
        assert_eq!(out, "12CL4K2eVqj7hQTix7dM7CVHCkpP17Pry3");
    }

    #[test]
    fn start_index_offsets_the_range() {
        let out = derive(ZPUB, "receive", 1, 1, "auto", "list", false).unwrap();
        assert_eq!(out, "bc1qnjg0jd8228aq7egyzacy8cys3knf9xvrerkf9g");
    }

    #[test]
    fn address_type_override_beats_the_prefix() {
        let out = derive(ZPUB, "receive", 1, 0, "p2pkh", "table", false).unwrap();
        assert!(out.contains("address_type: p2pkh"), "{out}");
        assert!(out.contains("m/0/0  1"), "{out}");
    }

    #[test]
    fn csv_format_has_a_header_and_optional_public_key() {
        let out = derive(ZPUB, "change", 1, 0, "auto", "csv", true).unwrap();
        assert_eq!(
            out,
            "chain,index,path,address,public_key\nchange,0,m/1/0,bc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el,03025324888e429ab8e3dbaf1f7802648b9cd01e9b418485c5fa4c1b9b5700e1a6"
        );
    }

    #[test]
    fn list_format_emits_one_address_per_line() {
        let out = derive(ZPUB, "both", 1, 0, "auto", "list", false).unwrap();
        assert_eq!(
            out,
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu\nbc1q8c6fshw2dlwun7ekn9qwf37cu2rn755upcp6el"
        );
    }

    #[test]
    fn derive_str_parses_blank_fields_as_defaults() {
        let out = derive_str(ZPUB, "receive", "", "", "", "list", "false").unwrap();
        assert_eq!(out.lines().count(), DEFAULT_COUNT as usize);
        assert_eq!(
            out.lines().next().unwrap(),
            "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu"
        );
    }

    #[test]
    fn derive_str_reads_checkbox_strings() {
        let out = derive_str(ZPUB, "receive", "1", "0", "auto", "csv", "true").unwrap();
        assert!(
            out.starts_with("chain,index,path,address,public_key"),
            "{out}"
        );
    }

    #[test]
    fn rejects_an_extended_private_key() {
        let e = derive(XPRV, "receive", 1, 0, "auto", "table", false).unwrap_err();
        assert!(e.contains("extended PRIVATE key"), "{e}");
    }

    #[test]
    fn rejects_a_malformed_key() {
        let e = derive("xpub-not-a-key", "receive", 1, 0, "auto", "table", false).unwrap_err();
        assert!(e.contains("invalid extended key"), "{e}");
    }

    #[test]
    fn rejects_an_empty_key() {
        let e = derive("   ", "receive", 1, 0, "auto", "table", false).unwrap_err();
        assert!(e.contains("xpub is required"), "{e}");
    }

    #[test]
    fn rejects_out_of_range_count() {
        let e = derive(ZPUB, "receive", 0, 0, "auto", "table", false).unwrap_err();
        assert!(e.contains("count must be between 1 and 100"), "{e}");
        let e = derive(ZPUB, "receive", 101, 0, "auto", "table", false).unwrap_err();
        assert!(e.contains("count must be between 1 and 100"), "{e}");
    }

    #[test]
    fn accepts_the_exact_count_cap() {
        let out = derive(ZPUB, "receive", MAX_COUNT, 0, "auto", "list", false).unwrap();
        assert_eq!(out.lines().count(), MAX_COUNT as usize);
    }

    #[test]
    fn rejects_a_hardened_start_index() {
        let e = derive(ZPUB, "receive", 1, MAX_INDEX + 1, "auto", "table", false).unwrap_err();
        assert!(e.contains("non-hardened index"), "{e}");
    }

    #[test]
    fn rejects_a_range_that_runs_past_the_last_index() {
        let e = derive(ZPUB, "receive", 2, MAX_INDEX, "auto", "table", false).unwrap_err();
        assert!(
            e.contains("runs past the highest non-hardened index"),
            "{e}"
        );
    }

    #[test]
    fn rejects_unknown_enum_values() {
        assert!(derive(ZPUB, "savings", 1, 0, "auto", "table", false)
            .unwrap_err()
            .contains("unknown chain"));
        assert!(derive(ZPUB, "receive", 1, 0, "auto", "json", false)
            .unwrap_err()
            .contains("unknown format"));
        assert!(derive(ZPUB, "receive", 1, 0, "p2tr", "table", false)
            .unwrap_err()
            .contains("unknown address_type"));
    }
}
