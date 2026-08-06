//! FTP control-channel parsing: learn each transfer's filename, direction, and
//! the negotiated data endpoint (PASV / EPSV / PORT / EPRT) so the matching
//! data connection can be named instead of dumped as an anonymous blob.

use crate::capture::Ip;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    /// Server → client (RETR, LIST, NLST).
    Download,
    /// Client → server (STOR, STOU, APPE).
    Upload,
}

#[derive(Clone, Debug)]
pub struct Transfer {
    /// Endpoint the data connection is opened TO.
    pub endpoint: Option<(Ip, u16)>,
    pub filename: String,
    pub dir: Dir,
}

fn lines(data: &[u8]) -> Vec<String> {
    // FTP control text is Latin-1 in practice; lossy UTF-8 is close enough for
    // command/reply parsing and never panics.
    String::from_utf8_lossy(data)
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

/// Does this conversation look like an FTP control channel?
pub fn is_control(c2s: &[u8], s2c: &[u8]) -> bool {
    let banner = s2c.starts_with(b"220") || s2c.starts_with(b"220-");
    let cmds = lines(c2s).iter().any(|l| {
        let u = l.to_ascii_uppercase();
        u.starts_with("USER ")
            || u.starts_with("RETR ")
            || u.starts_with("STOR ")
            || u.starts_with("PASV")
            || u.starts_with("EPSV")
    });
    banner && cmds || cmds && s2c.starts_with(b"2")
}

fn parse_h1_h6(list: &str) -> Option<(Ip, u16)> {
    let nums: Vec<u16> = list
        .split(',')
        .map(|s| s.trim().parse::<u16>().ok())
        .collect::<Option<Vec<_>>>()?;
    if nums.len() != 6 || nums[..4].iter().any(|n| *n > 255) {
        return None;
    }
    let ip = Ip::V4([nums[0] as u8, nums[1] as u8, nums[2] as u8, nums[3] as u8]);
    Some((ip, nums[4] * 256 + nums[5]))
}

/// `227 Entering Passive Mode (h1,h2,h3,h4,p1,p2)`
fn parse_227(line: &str) -> Option<(Ip, u16)> {
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    parse_h1_h6(&line[open + 1..close])
}

/// `229 ... (|||port|)`
fn parse_229(line: &str) -> Option<u16> {
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    let inner = &line[open + 1..close];
    let sep = inner.chars().next()?;
    inner.split(sep).filter(|s| !s.is_empty()).next_back()?.parse().ok()
}

/// `EPRT |1|192.0.2.1|1234|`
fn parse_eprt(arg: &str) -> Option<(Ip, u16)> {
    let sep = arg.chars().next()?;
    let parts: Vec<&str> = arg.split(sep).filter(|s| !s.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }
    let octets: Vec<u8> = parts[1].split('.').map(|s| s.parse().ok()).collect::<Option<Vec<_>>>()?;
    if octets.len() != 4 {
        return None;
    }
    Some((Ip::V4([octets[0], octets[1], octets[2], octets[3]]), parts[2].parse().ok()?))
}

/// Walk the control channel and return the transfers it announced, in order.
///
/// `server_ip` is used when a PASV reply advertises `0,0,0,0` (some servers
/// behind NAT do) and for EPSV, which only carries a port.
pub fn scan(c2s: &[u8], s2c: &[u8], server_ip: Ip) -> Vec<Transfer> {
    // Passive replies, consumed in order as PASV/EPSV commands are seen.
    let mut passive: Vec<(Option<Ip>, u16)> = Vec::new();
    for line in lines(s2c) {
        if line.starts_with("227") {
            if let Some((ip, port)) = parse_227(&line) {
                let ip = if matches!(ip, Ip::V4([0, 0, 0, 0])) { None } else { Some(ip) };
                passive.push((ip, port));
            }
        } else if line.starts_with("229") {
            if let Some(port) = parse_229(&line) {
                passive.push((None, port));
            }
        }
    }

    let mut out = Vec::new();
    let mut pending: Option<(Ip, u16)> = None;
    let mut passive_idx = 0usize;
    for line in lines(c2s) {
        let upper = line.trim().to_ascii_uppercase();
        let (cmd, arg) = match line.trim().split_once(' ') {
            Some((c, a)) => (c.to_ascii_uppercase(), a.trim().to_string()),
            None => (upper.clone(), String::new()),
        };
        match cmd.as_str() {
            "PASV" | "EPSV" => {
                if let Some((ip, port)) = passive.get(passive_idx).copied() {
                    passive_idx += 1;
                    pending = Some((ip.unwrap_or(server_ip), port));
                }
            }
            "PORT" => pending = parse_h1_h6(&arg),
            "EPRT" => pending = parse_eprt(&arg),
            "RETR" | "STOR" | "STOU" | "APPE" | "LIST" | "NLST" | "MLSD" => {
                let dir = if matches!(cmd.as_str(), "STOR" | "STOU" | "APPE") {
                    Dir::Upload
                } else {
                    Dir::Download
                };
                let name = if arg.is_empty() {
                    format!("{}.txt", cmd.to_ascii_lowercase())
                } else {
                    arg.clone()
                };
                out.push(Transfer { endpoint: pending.take(), filename: name, dir });
            }
            _ => {}
        }
    }
    out
}
