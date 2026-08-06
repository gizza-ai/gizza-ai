//! SMB2/3 object extraction over NBSS-framed TCP (ports 445 and 139).
//!
//! CREATE names a handle, the CREATE response binds it to a `FileId`, and the
//! READ/WRITE pairs carry the bytes at explicit file offsets — so an object is
//! assembled sparsely and its coverage reported, exactly like a packet analyser's
//! completeness column. SMB1/CIFS is detected but not carved (reported as a note).

use std::collections::BTreeMap;

/// Ceiling on a single reconstructed SMB object.
pub const MAX_OBJECT: usize = 16 * 1024 * 1024;

const CMD_CREATE: u16 = 5;
const CMD_READ: u16 = 8;
const CMD_WRITE: u16 = 9;
const FLAG_RESPONSE: u32 = 0x0000_0001;
const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;

fn u16l(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32l(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn u64l(b: &[u8], o: usize) -> Option<u64> {
    b.get(o..o + 8)
        .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

fn utf16le(b: &[u8]) -> String {
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

/// One SMB2 protocol data unit located inside a reassembled stream.
struct Pdu {
    /// Offset of the SMB2 header within the stream.
    header: usize,
    command: u16,
    is_response: bool,
    message_id: u64,
    body: usize,
}

/// Walk NBSS session messages and the compound chains inside them.
fn pdus(data: &[u8], smb1_seen: &mut bool) -> Vec<Pdu> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 4 <= data.len() && out.len() < 200_000 {
        let msg_type = data[pos];
        let len = ((data[pos + 1] as usize) << 16) | ((data[pos + 2] as usize) << 8) | data[pos + 3] as usize;
        let start = pos + 4;
        let end = start + len;
        if len == 0 || end > data.len() {
            break;
        }
        if msg_type == 0x00 {
            let mut h = start;
            // Compound chain: NextCommand is a delta from this header.
            for _ in 0..64 {
                match data.get(h..h + 4) {
                    Some(b"\xfeSMB") => {}
                    Some(b"\xffSMB") => {
                        *smb1_seen = true;
                        break;
                    }
                    _ => break,
                }
                let command = u16l(data, h + 12).unwrap_or(0);
                let flags = u32l(data, h + 16).unwrap_or(0);
                let next = u32l(data, h + 20).unwrap_or(0) as usize;
                let message_id = u64l(data, h + 24).unwrap_or(0);
                out.push(Pdu {
                    header: h,
                    command,
                    is_response: flags & FLAG_RESPONSE != 0,
                    message_id,
                    body: h + 64,
                });
                if next == 0 || h + next >= end {
                    break;
                }
                h += next;
            }
        }
        pos = end;
    }
    out
}

#[derive(Clone, Copy)]
struct Chunk {
    file_off: u64,
    /// Offset of the data inside its source stream.
    src: usize,
    len: usize,
    from_server: bool,
}

#[derive(Default)]
struct Handle {
    name: String,
    eof: u64,
    chunks: Vec<Chunk>,
    read_bytes: usize,
    write_bytes: usize,
}

pub struct Object {
    pub path: String,
    pub filename: String,
    pub data: Vec<u8>,
    /// Bytes actually present (max_end minus holes).
    pub covered: usize,
    /// `EndOfFile` reported by the server, or 0 when unknown.
    pub declared_size: u64,
    /// True when the bytes came from READ responses (a download).
    pub is_read: bool,
    /// Offset in the source stream of the first data byte, for packet attribution.
    pub first_offset: usize,
    pub first_from_server: bool,
}

/// Extract every SMB2 object carried by one conversation.
pub fn extract(c2s: &[u8], s2c: &[u8]) -> (Vec<Object>, bool) {
    let mut smb1 = false;
    let reqs = pdus(c2s, &mut smb1);
    let resps = pdus(s2c, &mut smb1);
    if reqs.is_empty() && resps.is_empty() {
        return (Vec::new(), smb1);
    }

    // MessageId → CREATE name requested by the client.
    let mut create_names: BTreeMap<u64, String> = BTreeMap::new();
    // MessageId → (FileId, offset) asked for by a READ.
    let mut read_reqs: BTreeMap<u64, ([u8; 16], u64)> = BTreeMap::new();
    let mut handles: BTreeMap<[u8; 16], Handle> = BTreeMap::new();

    for p in &reqs {
        if p.is_response {
            continue;
        }
        match p.command {
            CMD_CREATE => {
                let create_options = u32l(c2s, p.body + 40).unwrap_or(0);
                if create_options & FILE_DIRECTORY_FILE != 0 {
                    continue;
                }
                let name_off = u16l(c2s, p.body + 44).unwrap_or(0) as usize;
                let name_len = u16l(c2s, p.body + 46).unwrap_or(0) as usize;
                let s = p.header + name_off;
                if name_len > 0 && name_len < 4096 {
                    if let Some(raw) = c2s.get(s..s + name_len) {
                        create_names.insert(p.message_id, utf16le(raw));
                    }
                }
            }
            CMD_READ => {
                let mut fid = [0u8; 16];
                if let Some(raw) = c2s.get(p.body + 16..p.body + 32) {
                    fid.copy_from_slice(raw);
                    let off = u64l(c2s, p.body + 8).unwrap_or(0);
                    read_reqs.insert(p.message_id, (fid, off));
                }
            }
            CMD_WRITE => {
                let data_off = u16l(c2s, p.body + 2).unwrap_or(0) as usize;
                let len = u32l(c2s, p.body + 4).unwrap_or(0) as usize;
                let file_off = u64l(c2s, p.body + 8).unwrap_or(0);
                let mut fid = [0u8; 16];
                let Some(raw) = c2s.get(p.body + 16..p.body + 32) else { continue };
                fid.copy_from_slice(raw);
                let src = p.header + data_off;
                if len == 0 || len > MAX_OBJECT || src + len > c2s.len() {
                    continue;
                }
                let h = handles.entry(fid).or_default();
                h.write_bytes += len;
                h.chunks.push(Chunk { file_off, src, len, from_server: false });
            }
            _ => {}
        }
    }

    for p in &resps {
        if !p.is_response {
            continue;
        }
        match p.command {
            CMD_CREATE => {
                let status = u32l(s2c, p.header + 8).unwrap_or(0);
                if status != 0 {
                    continue;
                }
                let mut fid = [0u8; 16];
                let Some(raw) = s2c.get(p.body + 64..p.body + 80) else { continue };
                fid.copy_from_slice(raw);
                let eof = u64l(s2c, p.body + 48).unwrap_or(0);
                let name = create_names.get(&p.message_id).cloned().unwrap_or_default();
                let h = handles.entry(fid).or_default();
                if !name.is_empty() {
                    h.name = name;
                }
                if eof > 0 {
                    h.eof = eof;
                }
            }
            CMD_READ => {
                let status = u32l(s2c, p.header + 8).unwrap_or(0);
                if status != 0 {
                    continue;
                }
                let Some(&(fid, file_off)) = read_reqs.get(&p.message_id) else { continue };
                let data_off = *s2c.get(p.body + 2).unwrap_or(&0) as usize;
                let len = u32l(s2c, p.body + 4).unwrap_or(0) as usize;
                let src = p.header + data_off;
                if len == 0 || len > MAX_OBJECT || src + len > s2c.len() {
                    continue;
                }
                let h = handles.entry(fid).or_default();
                h.read_bytes += len;
                h.chunks.push(Chunk { file_off, src, len, from_server: true });
            }
            _ => {}
        }
    }

    let mut objects = Vec::new();
    for (_fid, mut h) in handles {
        if h.chunks.is_empty() {
            continue;
        }
        h.chunks.sort_by_key(|c| c.file_off);
        let max_end = h
            .chunks
            .iter()
            .map(|c| c.file_off.saturating_add(c.len as u64))
            .max()
            .unwrap_or(0)
            .min(MAX_OBJECT as u64) as usize;
        if max_end == 0 {
            continue;
        }
        let mut data = vec![0u8; max_end];
        let mut covered_end = 0usize;
        let mut present = 0usize;
        let mut first_offset = 0usize;
        let mut first_from_server = false;
        let mut first = true;
        for c in &h.chunks {
            let start = c.file_off as usize;
            let end = (start + c.len).min(max_end);
            if start >= max_end || end <= covered_end {
                continue;
            }
            let copy_from = covered_end.max(start);
            let skip = copy_from - start;
            let n = end - copy_from;
            let src_stream: &[u8] = if c.from_server { s2c } else { c2s };
            let Some(slice) = src_stream.get(c.src + skip..c.src + skip + n) else { continue };
            data[copy_from..copy_from + n].copy_from_slice(slice);
            present += n;
            if first {
                first_offset = c.src;
                first_from_server = c.from_server;
                first = false;
            }
            covered_end = end;
        }
        if present == 0 {
            continue;
        }
        let path = h.name.clone();
        let filename = path
            .rsplit(['\\', '/'])
            .next()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "smb-object.bin".to_string());
        objects.push(Object {
            path,
            filename,
            data,
            covered: present,
            declared_size: h.eof,
            is_read: h.read_bytes >= h.write_bytes,
            first_offset,
            first_from_server,
        });
    }
    (objects, smb1)
}
