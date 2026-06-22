//! gizza-ai/torrent-file-inspector core — pure, dependency-free `.torrent`
//! (bencode) parsing. Decodes the metainfo dictionary, computes the SHA-1
//! info-hash over the raw `info` value, and extracts the human-relevant fields:
//! name, file list with sizes, announce/announce-list trackers, piece length,
//! piece count, total size, comment, creator, and creation date.
//!
//! No I/O. The only dependency is `sha1` (instantiates clean in wasm32) for the
//! info-hash; bencode parsing is hand-rolled so there are no non-wasm transitive
//! deps. Runs on every backend including the chat Service Worker.

use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

/// A decoded bencode value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bencode {
    Int(i64),
    Bytes(Vec<u8>),
    List(Vec<Bencode>),
    /// Keys are raw byte strings; kept in sorted order (BTreeMap) which is the
    /// canonical bencode dictionary ordering.
    Dict(BTreeMap<Vec<u8>, Bencode>),
}

impl Bencode {
    fn as_int(&self) -> Option<i64> {
        match self {
            Bencode::Int(i) => Some(*i),
            _ => None,
        }
    }
    fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Bencode::Bytes(b) => Some(b),
            _ => None,
        }
    }
    fn as_list(&self) -> Option<&[Bencode]> {
        match self {
            Bencode::List(l) => Some(l),
            _ => None,
        }
    }
    fn as_dict(&self) -> Option<&BTreeMap<Vec<u8>, Bencode>> {
        match self {
            Bencode::Dict(d) => Some(d),
            _ => None,
        }
    }
    fn dict_get(&self, key: &[u8]) -> Option<&Bencode> {
        self.as_dict().and_then(|d| d.get(key))
    }
    fn dict_str(&self, key: &[u8]) -> Option<String> {
        self.dict_get(key)
            .and_then(|v| v.as_bytes())
            .map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// A streaming bencode parser over a byte slice. It records, for the top-level
/// `info` dictionary, the byte range of its raw encoding so we can hash it
/// exactly as it appeared (the info-hash must be over the original bytes).
struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Parser { data, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Result<Bencode, String> {
        match self.peek() {
            Some(b'i') => self.parse_int(),
            Some(b'l') => self.parse_list(),
            Some(b'd') => self.parse_dict(),
            Some(c) if c.is_ascii_digit() => self.parse_bytes().map(Bencode::Bytes),
            Some(c) => Err(format!("unexpected byte '{}' at offset {}", c as char, self.pos)),
            None => Err("unexpected end of input".into()),
        }
    }

    fn parse_int(&mut self) -> Result<Bencode, String> {
        // 'i' <digits> 'e'
        self.pos += 1; // consume 'i'
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b'e' {
                break;
            }
            self.pos += 1;
        }
        if self.peek() != Some(b'e') {
            return Err("unterminated integer".into());
        }
        let s = std::str::from_utf8(&self.data[start..self.pos])
            .map_err(|_| "non-utf8 integer".to_string())?;
        self.pos += 1; // consume 'e'
        let n: i64 = s.parse().map_err(|_| format!("invalid integer '{s}'"))?;
        Ok(Bencode::Int(n))
    }

    fn parse_bytes(&mut self) -> Result<Vec<u8>, String> {
        // <len> ':' <bytes>
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c == b':' {
                break;
            }
            if !c.is_ascii_digit() {
                return Err(format!("invalid length digit at offset {}", self.pos));
            }
            self.pos += 1;
        }
        if self.peek() != Some(b':') {
            return Err("byte-string missing ':'".into());
        }
        let len: usize = std::str::from_utf8(&self.data[start..self.pos])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or("invalid byte-string length")?;
        self.pos += 1; // consume ':'
        let end = self
            .pos
            .checked_add(len)
            .ok_or("byte-string length overflow")?;
        if end > self.data.len() {
            return Err("byte-string length exceeds input".into());
        }
        let out = self.data[self.pos..end].to_vec();
        self.pos = end;
        Ok(out)
    }

    fn parse_list(&mut self) -> Result<Bencode, String> {
        self.pos += 1; // consume 'l'
        let mut items = Vec::new();
        loop {
            match self.peek() {
                Some(b'e') => {
                    self.pos += 1;
                    break;
                }
                None => return Err("unterminated list".into()),
                _ => items.push(self.parse_value()?),
            }
        }
        Ok(Bencode::List(items))
    }

    fn parse_dict(&mut self) -> Result<Bencode, String> {
        self.pos += 1; // consume 'd'
        let mut map = BTreeMap::new();
        loop {
            match self.peek() {
                Some(b'e') => {
                    self.pos += 1;
                    break;
                }
                None => return Err("unterminated dictionary".into()),
                _ => {
                    let key = self.parse_bytes()?;
                    let val = self.parse_value()?;
                    map.insert(key, val);
                }
            }
        }
        Ok(Bencode::Dict(map))
    }

    /// Parse a dict at the top level, returning it plus the byte range of the
    /// raw `info` value (for the info-hash).
    fn parse_root(&mut self) -> Result<(Bencode, Option<(usize, usize)>), String> {
        if self.peek() != Some(b'd') {
            return Err("a .torrent file must be a bencode dictionary".into());
        }
        self.pos += 1; // consume 'd'
        let mut map = BTreeMap::new();
        let mut info_range = None;
        loop {
            match self.peek() {
                Some(b'e') => {
                    self.pos += 1;
                    break;
                }
                None => return Err("unterminated root dictionary".into()),
                _ => {
                    let key = self.parse_bytes()?;
                    let val_start = self.pos;
                    let val = self.parse_value()?;
                    if key == b"info" {
                        info_range = Some((val_start, self.pos));
                    }
                    map.insert(key, val);
                }
            }
        }
        Ok((Bencode::Dict(map), info_range))
    }
}

/// One file entry from the torrent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    /// Path components joined with '/'.
    pub path: String,
    /// Length in bytes.
    pub length: i64,
}

/// The extracted, human-relevant view of a `.torrent` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentInfo {
    /// Suggested name (single file's name, or the root directory for a multi-file torrent).
    pub name: String,
    /// Lowercase hex SHA-1 of the bencoded `info` dictionary (the BitTorrent v1 info-hash).
    pub info_hash: String,
    /// True when the torrent describes multiple files.
    pub is_multi_file: bool,
    /// The files in the torrent (one entry for a single-file torrent).
    pub files: Vec<TorrentFile>,
    /// Total size across all files, in bytes.
    pub total_length: i64,
    /// `piece length` from the info dict (bytes per piece).
    pub piece_length: i64,
    /// Number of pieces (length of the `pieces` field / 20).
    pub piece_count: usize,
    /// Whether the torrent is marked private (`info.private == 1`).
    pub private: bool,
    /// All tracker announce URLs (from `announce` and `announce-list`), deduped, in order.
    pub trackers: Vec<String>,
    /// `comment`, if present.
    pub comment: Option<String>,
    /// `created by`, if present.
    pub created_by: Option<String>,
    /// `creation date` as a Unix timestamp (seconds), if present.
    pub creation_date: Option<i64>,
}

/// Parse a `.torrent` file's raw bytes into a [`TorrentInfo`].
pub fn inspect(bytes: &[u8]) -> Result<TorrentInfo, String> {
    if bytes.is_empty() {
        return Err("the file is empty — nothing to inspect".into());
    }
    let mut parser = Parser::new(bytes);
    let (root, info_range) =
        parser.parse_root().map_err(|e| format!("not a valid .torrent (bencode) file: {e}"))?;

    let info = root
        .dict_get(b"info")
        .ok_or("not a .torrent: missing the 'info' dictionary")?;
    let (info_start, info_end) =
        info_range.ok_or("internal error: info dictionary range not recorded")?;

    // Info-hash = SHA-1 over the RAW bytes of the info value.
    let mut hasher = Sha1::new();
    hasher.update(&bytes[info_start..info_end]);
    let digest = hasher.finalize();
    let info_hash = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let name = info
        .dict_str(b"name")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(unnamed)".to_string());

    let piece_length = info.dict_get(b"piece length").and_then(|v| v.as_int()).unwrap_or(0);
    let piece_count = info
        .dict_get(b"pieces")
        .and_then(|v| v.as_bytes())
        .map(|b| b.len() / 20)
        .unwrap_or(0);
    let private = info.dict_get(b"private").and_then(|v| v.as_int()) == Some(1);

    // Files: single-file torrents have info.length; multi-file have info.files (a
    // list of {length, path:[components]}).
    let mut files = Vec::new();
    let is_multi_file;
    if let Some(file_list) = info.dict_get(b"files").and_then(|v| v.as_list()) {
        is_multi_file = true;
        for entry in file_list {
            let length = entry.dict_get(b"length").and_then(|v| v.as_int()).unwrap_or(0);
            let path = entry
                .dict_get(b"path")
                .and_then(|v| v.as_list())
                .map(|comps| {
                    comps
                        .iter()
                        .filter_map(|c| c.as_bytes())
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default();
            // Prefix with the torrent root directory name for clarity.
            let full = if path.is_empty() {
                name.clone()
            } else {
                format!("{name}/{path}")
            };
            files.push(TorrentFile { path: full, length });
        }
    } else {
        is_multi_file = false;
        let length = info.dict_get(b"length").and_then(|v| v.as_int()).unwrap_or(0);
        files.push(TorrentFile { path: name.clone(), length });
    }
    let total_length: i64 = files.iter().map(|f| f.length).sum();

    // Trackers: `announce` plus flattened `announce-list` (list of tiers), deduped.
    let mut trackers = Vec::new();
    let mut seen = std::collections::HashSet::new();
    fn push(t: String, trackers: &mut Vec<String>, seen: &mut std::collections::HashSet<String>) {
        if !t.is_empty() && seen.insert(t.clone()) {
            trackers.push(t);
        }
    }
    if let Some(a) = root.dict_str(b"announce") {
        push(a, &mut trackers, &mut seen);
    }
    if let Some(tiers) = root.dict_get(b"announce-list").and_then(|v| v.as_list()) {
        for tier in tiers {
            if let Some(urls) = tier.as_list() {
                for u in urls {
                    if let Some(b) = u.as_bytes() {
                        push(String::from_utf8_lossy(b).into_owned(), &mut trackers, &mut seen);
                    }
                }
            }
        }
    }

    let comment = root.dict_str(b"comment").filter(|s| !s.is_empty());
    let created_by = root.dict_str(b"created by").filter(|s| !s.is_empty());
    let creation_date = root.dict_get(b"creation date").and_then(|v| v.as_int());

    Ok(TorrentInfo {
        name,
        info_hash,
        is_multi_file,
        files,
        total_length,
        piece_length,
        piece_count,
        private,
        trackers,
        comment,
        created_by,
        creation_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal single-file torrent: `pieces` is 40 bytes (2 pieces of 20).
    fn single_file_torrent() -> Vec<u8> {
        let pieces: Vec<u8> = vec![0u8; 40];
        let mut info = Vec::new();
        info.extend_from_slice(b"d6:lengthi12345e4:name9:hello.txt12:piece lengthi16384e6:pieces40:");
        info.extend_from_slice(&pieces);
        info.push(b'e');

        let mut t = Vec::new();
        t.extend_from_slice(b"d8:announce23:http://tracker.test/ann4:info");
        t.extend_from_slice(&info);
        t.push(b'e');
        t
    }

    #[test]
    fn parses_single_file() {
        let bytes = single_file_torrent();
        let info = inspect(&bytes).unwrap();
        assert_eq!(info.name, "hello.txt");
        assert!(!info.is_multi_file);
        assert_eq!(info.files.len(), 1);
        assert_eq!(info.files[0].path, "hello.txt");
        assert_eq!(info.files[0].length, 12345);
        assert_eq!(info.total_length, 12345);
        assert_eq!(info.piece_length, 16384);
        assert_eq!(info.piece_count, 2);
        assert!(!info.private);
        assert_eq!(info.trackers, vec!["http://tracker.test/ann".to_string()]);
        assert_eq!(info.info_hash.len(), 40);
        assert!(info.info_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn info_hash_is_stable_and_correct() {
        let bytes = single_file_torrent();
        let info = inspect(&bytes).unwrap();
        let pieces: Vec<u8> = vec![0u8; 40];
        let mut expected_info = Vec::new();
        expected_info
            .extend_from_slice(b"d6:lengthi12345e4:name9:hello.txt12:piece lengthi16384e6:pieces40:");
        expected_info.extend_from_slice(&pieces);
        expected_info.push(b'e');
        let mut h = Sha1::new();
        h.update(&expected_info);
        let expected: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(info.info_hash, expected);
    }

    #[test]
    fn parses_multi_file_with_announce_list_and_meta() {
        let pieces: Vec<u8> = vec![0u8; 20];
        let mut info = Vec::new();
        info.extend_from_slice(
            b"d5:filesld6:lengthi100e4:pathl5:a.txteed6:lengthi200e4:pathl3:sub5:b.txteee",
        );
        info.extend_from_slice(b"4:name5:mydir12:piece lengthi32768e7:privatei1e6:pieces20:");
        info.extend_from_slice(&pieces);
        info.push(b'e');

        let mut t = Vec::new();
        t.extend_from_slice(b"d8:announce11:http://a/an");
        t.extend_from_slice(b"13:announce-listll11:http://a/anel11:http://b/anee");
        t.extend_from_slice(b"7:comment5:hello10:created by6:mktorr13:creation datei1600000000e");
        t.extend_from_slice(b"4:info");
        t.extend_from_slice(&info);
        t.push(b'e');

        let r = inspect(&t).unwrap();
        assert_eq!(r.name, "mydir");
        assert!(r.is_multi_file);
        assert_eq!(r.files.len(), 2);
        assert_eq!(r.files[0].path, "mydir/a.txt");
        assert_eq!(r.files[0].length, 100);
        assert_eq!(r.files[1].path, "mydir/sub/b.txt");
        assert_eq!(r.files[1].length, 200);
        assert_eq!(r.total_length, 300);
        assert_eq!(r.piece_length, 32768);
        assert_eq!(r.piece_count, 1);
        assert!(r.private);
        assert_eq!(
            r.trackers,
            vec!["http://a/an".to_string(), "http://b/an".to_string()]
        );
        assert_eq!(r.comment.as_deref(), Some("hello"));
        assert_eq!(r.created_by.as_deref(), Some("mktorr"));
        assert_eq!(r.creation_date, Some(1600000000));
    }

    #[test]
    fn empty_is_error() {
        assert!(inspect(&[]).is_err());
    }

    #[test]
    fn non_torrent_is_error() {
        assert!(inspect(b"not a torrent at all").is_err());
        assert!(inspect(b"d3:foo3:bare").is_err());
    }

    #[test]
    fn bencode_primitives() {
        assert_eq!(Parser::new(b"i42e").parse_value().unwrap(), Bencode::Int(42));
        assert_eq!(Parser::new(b"i-7e").parse_value().unwrap(), Bencode::Int(-7));
        assert_eq!(
            Parser::new(b"4:spam").parse_value().unwrap(),
            Bencode::Bytes(b"spam".to_vec())
        );
        assert_eq!(
            Parser::new(b"l4:spami42ee").parse_value().unwrap(),
            Bencode::List(vec![Bencode::Bytes(b"spam".to_vec()), Bencode::Int(42)])
        );
        assert!(Parser::new(b"i42").parse_value().is_err());
        assert!(Parser::new(b"5:ab").parse_value().is_err());
    }
}
