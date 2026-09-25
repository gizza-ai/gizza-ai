//! gizza-ai/file-hash-dedupe core — content-address a set of files and report
//! byte-identical duplicates regardless of filename. No wafer/wasm-bindgen
//! deps; pure compute, so it runs on every backend (chat Service Worker +
//! native CLI).
//!
//! Design notes:
//!   - The caller hashes each file's bytes as they are resolved (one file in
//!     memory at a time) via [`digest_file`], then hands this module a compact
//!     [`FileEntry`] per file. Nothing here holds file contents, so a 50-file
//!     set costs a few kilobytes rather than gigabytes.
//!   - Each file carries TWO digests: the user-chosen `hash` (reported) and an
//!     internal SHA-256 `confirm` digest. Identity is `(bytes, confirm)`, so a
//!     weak choice like CRC-32 or MD5 can be reported without ever creating a
//!     false duplicate. When the chosen digest collides but the confirmation
//!     digest disagrees, the files stay in separate groups and the event is
//!     counted in [`Summary::hash_collisions`].
//!   - Groups are exact-match clusters (not transitive-closure clusters like a
//!     perceptual matcher needs) — byte equality is an equivalence relation, so
//!     one bucket per distinct content is exactly right.

use std::collections::BTreeMap;

use blake3::Hasher as Blake3;
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

use serde::Serialize;

/// Hash algorithm used for the REPORTED `hash` field. Identity is always
/// confirmed with SHA-256 on top of this, so the weak options are safe.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
    Blake3,
    Crc32,
}

impl Algorithm {
    /// Parse the descriptor's `algorithm` enum value (case-insensitive, `-`/`_`
    /// tolerant so `sha-256` and `sha_256` also work from a CLI).
    pub fn parse(s: &str) -> Result<Self, String> {
        let norm: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| *c != '-' && *c != '_')
            .collect();
        match norm.as_str() {
            "md5" => Ok(Self::Md5),
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            "sha512" => Ok(Self::Sha512),
            "blake3" => Ok(Self::Blake3),
            "crc32" => Ok(Self::Crc32),
            _ => Err(format!(
                "unknown algorithm {s:?} — use one of: sha256, sha1, md5, sha512, blake3, crc32"
            )),
        }
    }

    /// Canonical name echoed back in the report.
    pub fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
            Self::Blake3 => "blake3",
            Self::Crc32 => "crc32",
        }
    }
}

/// Which copy of a duplicate group to suggest keeping. Every member is
/// byte-identical, so the choice is purely about which path/name to retain.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Keep {
    /// Lowest input index — the first one listed.
    First,
    /// Highest input index — the last one listed.
    Last,
    /// Shortest `source` label (ties → lowest index). Prefers `photo.jpg` over
    /// `photo (copy) (1).jpg`.
    ShortestName,
}

impl Keep {
    pub fn parse(s: &str) -> Result<Self, String> {
        let norm: String = s
            .trim()
            .to_ascii_lowercase()
            .chars()
            .filter(|c| *c != '-' && *c != '_' && *c != ' ')
            .collect();
        match norm.as_str() {
            "first" => Ok(Self::First),
            "last" => Ok(Self::Last),
            "shortestname" => Ok(Self::ShortestName),
            _ => Err(format!(
                "unknown keep policy {s:?} — use one of: first, last, shortest-name"
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Last => "last",
            Self::ShortestName => "shortest-name",
        }
    }
}

/// One already-hashed file. `hash` is in the user-chosen algorithm; `confirm`
/// is the internal SHA-256 used to prove byte identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Human-facing label (filename or URL) used to identify what to delete.
    pub label: String,
    pub bytes: usize,
    pub hash: String,
    pub confirm: String,
}

/// Hash `data` once, returning `(chosen-algorithm hex, sha256 confirmation hex)`.
/// When the chosen algorithm IS SHA-256 the digest is computed a single time.
pub fn digest_file(data: &[u8], algorithm: Algorithm) -> (String, String) {
    let confirm = hex(&Sha256::digest(data));
    let chosen = match algorithm {
        Algorithm::Sha256 => confirm.clone(),
        Algorithm::Md5 => hex(&Md5::digest(data)),
        Algorithm::Sha1 => hex(&Sha1::digest(data)),
        Algorithm::Sha512 => hex(&Sha512::digest(data)),
        Algorithm::Blake3 => {
            let mut h = Blake3::new();
            h.update(data);
            hex(h.finalize().as_bytes())
        }
        Algorithm::Crc32 => format!("{:08x}", crc32(data)),
    };
    (chosen, confirm)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// CRC-32 (IEEE 802.3, reflected) — the variant used by zip, gzip and PNG.
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

/// One reported file row.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FileInfo {
    /// Position in the input list (0-based) — the id `groups` reference.
    pub index: usize,
    pub source: String,
    pub bytes: usize,
    /// Digest in the requested algorithm, lowercase hex.
    pub hash: String,
    /// Index into `groups` when this file has duplicates, else `null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<usize>,
}

/// A set of byte-identical files with a keep/delete suggestion.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Group {
    /// Shared digest in the requested algorithm.
    pub hash: String,
    /// Shared size in bytes (identical for every member).
    pub bytes: usize,
    /// How many copies exist.
    pub count: usize,
    /// Member input indices, ascending.
    pub members: Vec<usize>,
    /// Suggested copy to keep, per the `keep` policy.
    pub keep: usize,
    /// Suggested copies to delete (every member except `keep`), ascending.
    pub delete: Vec<usize>,
    /// Bytes freed by deleting every copy but `keep`.
    pub reclaimable_bytes: usize,
}

/// Roll-up counts.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Summary {
    /// Distinct contents seen (= number of unique files after dedupe).
    pub distinct_files: usize,
    /// Files appearing exactly once.
    pub unique_files: usize,
    /// Redundant copies — the total length of every group's `delete` list.
    pub duplicate_files: usize,
    pub duplicate_groups: usize,
    pub total_bytes: usize,
    pub bytes_reclaimable: usize,
    /// Percent of the input bytes that is redundant, one decimal.
    pub wasted_percent: f64,
    /// Files whose chosen-algorithm digest matched another file's while the
    /// SHA-256 confirmation disagreed. Always 0 for sha256/sha512/blake3 in
    /// practice; a real possibility for crc32.
    pub hash_collisions: usize,
}

/// Full result.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    pub algorithm: String,
    pub keep_policy: String,
    pub file_count: usize,
    /// Every file when `include_unique`, else only files that have a duplicate.
    pub files: Vec<FileInfo>,
    pub groups: Vec<Group>,
    pub summary: Summary,
}

/// Hard cap on the number of files in one call — keeps a chat-side request from
/// turning into an unbounded fetch loop.
pub const MAX_FILES: usize = 50;

/// Content-address `entries` and report duplicate groups.
///
/// Two files are duplicates iff they have the same size AND the same SHA-256
/// confirmation digest. `include_unique` controls whether the `files` list also
/// carries the files that have no duplicate (the groups are unaffected).
pub fn dedupe(
    entries: &[FileEntry],
    algorithm: Algorithm,
    keep: Keep,
    include_unique: bool,
) -> Result<Report, String> {
    if entries.len() < 2 {
        return Err(format!(
            "need at least 2 files to compare, got {}",
            entries.len()
        ));
    }
    if entries.len() > MAX_FILES {
        return Err(format!(
            "too many files: {} exceeds the {MAX_FILES}-file cap",
            entries.len()
        ));
    }

    // Identity bucket = (size, sha256). BTreeMap keeps output deterministic;
    // buckets are then ordered by their lowest member index so the report reads
    // in input order.
    let mut buckets: BTreeMap<(usize, &str), Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        buckets
            .entry((e.bytes, e.confirm.as_str()))
            .or_default()
            .push(i);
    }

    // A collision is a REPORTED digest shared by two files that are not the
    // same content. Count the files caught in one.
    let mut by_reported: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        by_reported.entry(e.hash.as_str()).or_default().push(i);
    }
    let mut hash_collisions = 0usize;
    for (_, idxs) in by_reported.iter() {
        if idxs.len() < 2 {
            continue;
        }
        let distinct: std::collections::BTreeSet<(usize, &str)> = idxs
            .iter()
            .map(|&i| (entries[i].bytes, entries[i].confirm.as_str()))
            .collect();
        if distinct.len() > 1 {
            hash_collisions += idxs.len();
        }
    }

    let mut dup_buckets: Vec<Vec<usize>> = buckets
        .into_values()
        .filter(|members| members.len() >= 2)
        .collect();
    dup_buckets.sort_by_key(|m| m[0]);

    let mut groups: Vec<Group> = Vec::with_capacity(dup_buckets.len());
    let mut group_of: BTreeMap<usize, usize> = BTreeMap::new();
    let mut duplicate_files = 0usize;
    let mut bytes_reclaimable = 0usize;

    for (gi, members) in dup_buckets.into_iter().enumerate() {
        let keep_idx = pick_keep(&members, entries, keep);
        let delete: Vec<usize> = members.iter().copied().filter(|&m| m != keep_idx).collect();
        let reclaimable: usize = delete.iter().map(|&m| entries[m].bytes).sum();
        duplicate_files += delete.len();
        bytes_reclaimable += reclaimable;
        for &m in &members {
            group_of.insert(m, gi);
        }
        groups.push(Group {
            hash: entries[members[0]].hash.clone(),
            bytes: entries[members[0]].bytes,
            count: members.len(),
            members,
            keep: keep_idx,
            delete,
            reclaimable_bytes: reclaimable,
        });
    }

    let files: Vec<FileInfo> = entries
        .iter()
        .enumerate()
        .filter(|(i, _)| include_unique || group_of.contains_key(i))
        .map(|(i, e)| FileInfo {
            index: i,
            source: e.label.clone(),
            bytes: e.bytes,
            hash: e.hash.clone(),
            group: group_of.get(&i).copied(),
        })
        .collect();

    let total_bytes: usize = entries.iter().map(|e| e.bytes).sum();
    let distinct_files = entries.len() - duplicate_files;
    let unique_files = entries.len() - group_of.len();
    let wasted_percent = if total_bytes == 0 {
        0.0
    } else {
        round1(100.0 * bytes_reclaimable as f64 / total_bytes as f64)
    };

    Ok(Report {
        algorithm: algorithm.name().to_string(),
        keep_policy: keep.name().to_string(),
        file_count: entries.len(),
        files,
        summary: Summary {
            distinct_files,
            unique_files,
            duplicate_files,
            duplicate_groups: groups.len(),
            total_bytes,
            bytes_reclaimable,
            wasted_percent,
            hash_collisions,
        },
        groups,
    })
}

/// Apply the keep policy to one group's members (already ascending).
fn pick_keep(members: &[usize], entries: &[FileEntry], keep: Keep) -> usize {
    match keep {
        Keep::First => members[0],
        Keep::Last => members[members.len() - 1],
        // Shortest label wins; ties fall back to the lowest index.
        Keep::ShortestName => *members
            .iter()
            .min_by_key(|&&m| (entries[m].label.chars().count(), m))
            .expect("groups always have >=2 members"),
    }
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(label: &str, bytes: usize, hash: &str, confirm: &str) -> FileEntry {
        FileEntry {
            label: label.to_string(),
            bytes,
            hash: hash.to_string(),
            confirm: confirm.to_string(),
        }
    }

    /// Build an entry from real bytes so the test exercises `digest_file` too.
    fn from_bytes(label: &str, data: &[u8], algorithm: Algorithm) -> FileEntry {
        let (hash, confirm) = digest_file(data, algorithm);
        FileEntry {
            label: label.to_string(),
            bytes: data.len(),
            hash,
            confirm,
        }
    }

    #[test]
    fn identical_bytes_group_regardless_of_filename() {
        let a = b"the same content";
        let entries = vec![
            from_bytes("report-final.txt", a, Algorithm::Sha256),
            from_bytes("copy of report (1).txt", a, Algorithm::Sha256),
            from_bytes("different.txt", b"something else", Algorithm::Sha256),
        ];
        let r = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap();

        assert_eq!(r.algorithm, "sha256");
        assert_eq!(r.keep_policy, "first");
        assert_eq!(r.file_count, 3);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].members, vec![0, 1]);
        assert_eq!(r.groups[0].count, 2);
        assert_eq!(r.groups[0].keep, 0);
        assert_eq!(r.groups[0].delete, vec![1]);
        assert_eq!(r.groups[0].bytes, a.len());
        assert_eq!(r.groups[0].reclaimable_bytes, a.len());
        assert_eq!(
            r.groups[0].hash,
            digest_file(a, Algorithm::Sha256).0,
            "the group carries the shared digest"
        );

        // include_unique = false → only the duplicated files are listed.
        assert_eq!(r.files.len(), 2);
        assert_eq!(r.files[0].index, 0);
        assert_eq!(r.files[0].source, "report-final.txt");
        assert_eq!(r.files[0].group, Some(0));
        assert_eq!(r.files[1].index, 1);

        assert_eq!(r.summary.duplicate_groups, 1);
        assert_eq!(r.summary.duplicate_files, 1);
        assert_eq!(r.summary.unique_files, 1);
        assert_eq!(r.summary.distinct_files, 2);
        assert_eq!(r.summary.total_bytes, a.len() * 2 + 14);
        assert_eq!(r.summary.bytes_reclaimable, a.len());
        assert_eq!(r.summary.hash_collisions, 0);
    }

    #[test]
    fn include_unique_lists_every_file_with_null_group() {
        let a = b"dup";
        let entries = vec![
            from_bytes("a.bin", a, Algorithm::Sha256),
            from_bytes("b.bin", a, Algorithm::Sha256),
            from_bytes("solo.bin", b"solo", Algorithm::Sha256),
        ];
        let r = dedupe(&entries, Algorithm::Sha256, Keep::First, true).unwrap();
        assert_eq!(r.files.len(), 3);
        assert_eq!(r.files[2].source, "solo.bin");
        assert_eq!(r.files[2].group, None, "a unique file has no group");
        assert_eq!(r.summary.unique_files, 1);
    }

    #[test]
    fn keep_policies_pick_different_copies() {
        let a = b"same";
        let entries = vec![
            from_bytes("longest-name-of-them-all.txt", a, Algorithm::Sha256),
            from_bytes("mid-name.txt", a, Algorithm::Sha256),
            from_bytes("s.txt", a, Algorithm::Sha256),
        ];
        let first = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap();
        assert_eq!(first.groups[0].keep, 0);
        assert_eq!(first.groups[0].delete, vec![1, 2]);

        let last = dedupe(&entries, Algorithm::Sha256, Keep::Last, false).unwrap();
        assert_eq!(last.groups[0].keep, 2);
        assert_eq!(last.groups[0].delete, vec![0, 1]);

        let short = dedupe(&entries, Algorithm::Sha256, Keep::ShortestName, false).unwrap();
        assert_eq!(short.groups[0].keep, 2, "s.txt is the shortest label");
        assert_eq!(short.groups[0].delete, vec![0, 1]);
    }

    #[test]
    fn same_size_different_content_never_groups() {
        // Equal length, different bytes — the classic size-only false positive.
        let entries = vec![
            from_bytes("a.bin", b"aaaa", Algorithm::Sha256),
            from_bytes("b.bin", b"bbbb", Algorithm::Sha256),
        ];
        let r = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap();
        assert!(r.groups.is_empty());
        assert_eq!(r.summary.bytes_reclaimable, 0);
        assert_eq!(r.summary.wasted_percent, 0.0);
        assert!(r.files.is_empty(), "nothing duplicated → nothing listed");
    }

    #[test]
    fn weak_digest_collision_is_detected_not_grouped() {
        // Two files whose REPORTED (crc32) digest matches while the SHA-256
        // confirmation disagrees: they must stay ungrouped and be counted.
        let entries = vec![
            entry("x.bin", 8, "deadbeef", "aa".repeat(32).as_str()),
            entry("y.bin", 8, "deadbeef", "bb".repeat(32).as_str()),
            entry("z.bin", 8, "cafebabe", "cc".repeat(32).as_str()),
        ];
        let r = dedupe(&entries, Algorithm::Crc32, Keep::First, false).unwrap();
        assert!(r.groups.is_empty(), "a collision is not a duplicate");
        assert_eq!(r.summary.hash_collisions, 2);
        assert_eq!(r.summary.duplicate_files, 0);
        assert_eq!(r.algorithm, "crc32");
    }

    #[test]
    fn wasted_percent_and_multi_group_counts() {
        let entries = vec![
            from_bytes("a1", b"1111", Algorithm::Sha256),
            from_bytes("a2", b"1111", Algorithm::Sha256),
            from_bytes("b1", b"2222", Algorithm::Sha256),
            from_bytes("b2", b"2222", Algorithm::Sha256),
        ];
        let r = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap();
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.summary.duplicate_groups, 2);
        assert_eq!(r.summary.duplicate_files, 2);
        assert_eq!(r.summary.distinct_files, 2);
        assert_eq!(r.summary.total_bytes, 16);
        assert_eq!(r.summary.bytes_reclaimable, 8);
        assert_eq!(r.summary.wasted_percent, 50.0);
    }

    #[test]
    fn known_digest_vectors_for_every_algorithm() {
        // "abc" — published test vectors, so the reported hash is verifiable.
        let d = |a: Algorithm| digest_file(b"abc", a).0;
        assert_eq!(d(Algorithm::Md5), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(d(Algorithm::Sha1), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            d(Algorithm::Sha256),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            d(Algorithm::Sha512),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            d(Algorithm::Blake3),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
        assert_eq!(d(Algorithm::Crc32), "352441c2");

        // The confirmation digest is always SHA-256, whatever was requested.
        assert_eq!(
            digest_file(b"abc", Algorithm::Crc32).1,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empty_files_are_duplicates_of_each_other() {
        let entries = vec![
            from_bytes("empty-a", b"", Algorithm::Sha256),
            from_bytes("empty-b", b"", Algorithm::Sha256),
        ];
        let r = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap();
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.summary.total_bytes, 0);
        assert_eq!(r.summary.wasted_percent, 0.0, "no division by zero");
    }

    #[test]
    fn too_few_files_errors() {
        let entries = vec![from_bytes("only.bin", b"x", Algorithm::Sha256)];
        let err = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap_err();
        assert!(err.contains("at least 2"), "got: {err}");
    }

    #[test]
    fn too_many_files_errors_at_the_cap() {
        let entries: Vec<FileEntry> = (0..(MAX_FILES + 1))
            .map(|i| from_bytes(&format!("f{i}"), format!("{i}").as_bytes(), Algorithm::Sha256))
            .collect();
        let err = dedupe(&entries, Algorithm::Sha256, Keep::First, false).unwrap_err();
        assert!(err.contains("50-file cap"), "got: {err}");

        // …and exactly at the cap it succeeds.
        let ok: Vec<FileEntry> = entries[..MAX_FILES].to_vec();
        assert!(dedupe(&ok, Algorithm::Sha256, Keep::First, false).is_ok());
    }

    #[test]
    fn algorithm_and_keep_parsing() {
        assert_eq!(Algorithm::parse("SHA-256").unwrap(), Algorithm::Sha256);
        assert_eq!(Algorithm::parse("blake3").unwrap(), Algorithm::Blake3);
        assert_eq!(Keep::parse("shortest-name").unwrap(), Keep::ShortestName);
        assert_eq!(Keep::parse("Last").unwrap(), Keep::Last);

        let err = Algorithm::parse("sha384").unwrap_err();
        assert!(err.contains("unknown algorithm"), "got: {err}");
        let err = Keep::parse("biggest").unwrap_err();
        assert!(err.contains("unknown keep policy"), "got: {err}");
    }
}
