//! port-process-mapper core — turn `lsof -i`, `ss -tulpn`, `netstat -tulpn` (Linux) or
//! `netstat -ano` / `-anb` (Windows) output into one normalised port → PID → process
//! table, and flag ports that more than one process is bound to.
//!
//! Pure compute, shared by the chat skill block, the CLI and the web page. No I/O:
//! the caller pastes output that was captured elsewhere.

/// Maximum number of input lines accepted. Pasting more than this is almost always a
/// whole log file rather than a socket listing, and the table stops being readable.
pub const MAX_LINES: usize = 20_000;

/// How many distinct ports the "how do I free this port" command list covers
/// before it is truncated. Past this the list stops being a checklist.
pub const MAX_KILL_SUGGESTIONS: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    Auto,
    Lsof,
    Ss,
    Netstat,
    NetstatWindows,
}

impl InputFormat {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "lsof" => InputFormat::Lsof,
            "ss" => InputFormat::Ss,
            "netstat" => InputFormat::Netstat,
            "netstat-windows" | "netstat_windows" | "windows" => InputFormat::NetstatWindows,
            _ => InputFormat::Auto,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            InputFormat::Auto => "auto",
            InputFormat::Lsof => "lsof",
            InputFormat::Ss => "ss",
            InputFormat::Netstat => "netstat",
            InputFormat::NetstatWindows => "netstat-windows",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Markdown,
    Csv,
    Json,
    Text,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => OutputFormat::Csv,
            "json" => OutputFormat::Json,
            "text" => OutputFormat::Text,
            _ => OutputFormat::Markdown,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    Port,
    Pid,
    Process,
    State,
    Address,
}

impl SortBy {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "pid" => SortBy::Pid,
            "process" => SortBy::Process,
            "state" => SortBy::State,
            "address" => SortBy::Address,
            _ => SortBy::Port,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    Any,
    Tcp,
    Udp,
}

impl Protocol {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            _ => Protocol::Any,
        }
    }
}

pub struct Options {
    pub input_format: InputFormat,
    pub output_format: OutputFormat,
    pub sort_by: SortBy,
    pub listening_only: bool,
    pub protocol: Protocol,
    /// Comma-separated ports and ranges, e.g. `80,443,8000-8100`. Empty = every port.
    pub ports: String,
    /// Case-insensitive substring match on the process/command name. Empty = every process.
    pub process: String,
    pub conflicts_only: bool,
    /// Add a Service column naming the well-known service for each port number.
    pub annotate_services: bool,
    /// Append ready-to-run `kill` / `taskkill` commands for the listed ports.
    pub kill_commands: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            output_format: OutputFormat::Markdown,
            sort_by: SortBy::Port,
            listening_only: true,
            protocol: Protocol::Any,
            ports: String::new(),
            process: String::new(),
            conflicts_only: false,
            annotate_services: true,
            kill_commands: false,
        }
    }
}

/// The well-known service behind a port number, or `None` for an unregistered one.
///
/// Covers the IANA registrations an operator actually meets on a host plus the
/// unregistered development-server ports that are usually the real answer to
/// "what is this?" (3000, 5173, 8080, …). Protocol-agnostic: a handful of numbers
/// mean different things over TCP and UDP, and the label names the common case.
pub fn service_name(port: u32) -> Option<&'static str> {
    Some(match port {
        20 => "ftp-data",
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "domain (DNS)",
        67 => "dhcp-server",
        68 => "dhcp-client",
        69 => "tftp",
        80 => "http",
        88 => "kerberos",
        110 => "pop3",
        111 => "rpcbind",
        119 => "nntp",
        123 => "ntp",
        135 => "msrpc",
        137 => "netbios-ns",
        138 => "netbios-dgm",
        139 => "netbios-ssn",
        143 => "imap",
        161 => "snmp",
        162 => "snmp-trap",
        179 => "bgp",
        389 => "ldap",
        443 => "https",
        445 => "microsoft-ds (SMB)",
        465 => "smtps",
        500 => "isakmp (IPsec)",
        514 => "syslog",
        515 => "printer (LPD)",
        548 => "afp",
        587 => "submission (SMTP)",
        631 => "ipp (CUPS)",
        636 => "ldaps",
        873 => "rsync",
        902 => "vmware-auth",
        993 => "imaps",
        995 => "pop3s",
        1080 => "socks",
        1194 => "openvpn",
        1433 => "ms-sql-s",
        1521 => "oracle-tns",
        1701 => "l2tp",
        1723 => "pptp",
        1883 => "mqtt",
        1900 => "ssdp (UPnP)",
        2049 => "nfs",
        2181 => "zookeeper",
        2375 => "docker (plaintext)",
        2376 => "docker (TLS)",
        2379 => "etcd-client",
        2380 => "etcd-peer",
        3000 => "dev server (Node/Rails/Grafana)",
        3001 => "dev server (alternate)",
        3128 => "squid-http",
        3306 => "mysql",
        3389 => "ms-wbt-server (RDP)",
        4000 => "dev server (Phoenix/Jekyll)",
        4200 => "dev server (Angular)",
        4369 => "epmd (Erlang)",
        5000 => "dev server (Flask) / AirPlay on macOS",
        5001 => "dev server (alternate) / AirPlay on macOS",
        5060 => "sip",
        5061 => "sip-tls",
        5173 => "dev server (Vite)",
        5222 => "xmpp-client",
        5353 => "mdns (Bonjour)",
        5355 => "llmnr",
        5432 => "postgresql",
        5555 => "adb / dev server",
        5601 => "kibana",
        5672 => "amqp (RabbitMQ)",
        5900 => "vnc",
        5984 => "couchdb",
        6000 => "x11",
        6379 => "redis",
        6443 => "kubernetes-api",
        6667 => "irc",
        7000 => "dev server / AirPlay on macOS",
        7474 => "neo4j-http",
        8000 => "http-alt (dev server)",
        8008 => "http-alt",
        8080 => "http-alt (dev server/proxy)",
        8081 => "http-alt (alternate)",
        8086 => "influxdb",
        8125 => "statsd",
        8443 => "https-alt",
        8883 => "mqtts",
        8888 => "http-alt (Jupyter)",
        9000 => "php-fpm / MinIO / SonarQube",
        9090 => "prometheus",
        9092 => "kafka",
        9200 => "elasticsearch",
        9229 => "node-inspector",
        9418 => "git",
        10250 => "kubelet",
        11211 => "memcached",
        15672 => "rabbitmq-management",
        25565 => "minecraft",
        27017 => "mongodb",
        32400 => "plex",
        _ => return None,
    })
}

/// One socket, normalised across all four input dialects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SocketRow {
    /// `tcp` or `udp` (or whatever the source called it, lowercased).
    pub proto: String,
    pub ipv6: bool,
    /// Local address without the port (`0.0.0.0`, `127.0.0.1`, `::`, `*`).
    pub address: String,
    /// Local port as printed — numeric, a service name (`ssh`), or `*`.
    pub port: String,
    pub port_num: Option<u32>,
    /// Normalised state: `LISTEN`, `UNCONN`, `ESTABLISHED`, … or empty when the
    /// source prints no state column (UDP rows in netstat).
    pub state: String,
    pub pid: Option<u32>,
    pub process: String,
    pub user: String,
    /// Remote endpoint for connected sockets, empty for listeners/wildcards.
    pub peer: String,
}

impl SocketRow {
    pub fn proto_display(&self) -> String {
        if self.ipv6 {
            format!("{}6", self.proto)
        } else {
            self.proto.clone()
        }
    }
    pub fn is_listening(&self) -> bool {
        match self.state.as_str() {
            "LISTEN" | "UNCONN" => true,
            "" => self.proto == "udp",
            _ => false,
        }
    }
    /// Well-known service for this row's port, or `""` when the port is unregistered.
    pub fn service(&self) -> &'static str {
        self.port_num.and_then(service_name).unwrap_or("")
    }
}

/// One port bound by more than one distinct PID.
#[derive(Clone, Debug)]
pub struct Conflict {
    pub proto: String,
    pub port: String,
    pub holders: Vec<(Option<u32>, String, String)>, // (pid, process, address)
}

/// Parse `input` and render it in the requested output format.
pub fn parse(input: &str, opts: Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty: paste the output of `lsof -i -P -n`, `ss -tulpn`, \
                    `netstat -tulpn` (Linux) or `netstat -ano` (Windows)"
            .into());
    }
    let line_count = input.lines().count();
    if line_count > MAX_LINES {
        return Err(format!(
            "input has {line_count} lines, which is over the {MAX_LINES}-line limit: \
             paste just the socket listing, not a whole log file"
        ));
    }

    let ranges = parse_port_filter(&opts.ports)?;

    let detected = match opts.input_format {
        InputFormat::Auto => detect(input).ok_or_else(|| {
            "could not detect the input format: expected `lsof -i` (COMMAND/PID/NAME columns), \
             `ss -tulpn` (Netid/State/users:((…))), `netstat -tulpn` (Linux) or `netstat -ano` \
             (Windows). Set format=lsof|ss|netstat|netstat-windows to force one."
                .to_string()
        })?,
        forced => forced,
    };

    let mut rows = match detected {
        InputFormat::Lsof => parse_lsof(input),
        InputFormat::Ss => parse_ss(input),
        InputFormat::Netstat => parse_netstat(input),
        InputFormat::NetstatWindows => parse_netstat_windows(input),
        InputFormat::Auto => unreachable!("detect() never returns Auto"),
    };

    if rows.is_empty() {
        return Err(format!(
            "no socket rows found while parsing this as {} output: check the format, or paste \
             the command's data lines (not just its header)",
            detected.label()
        ));
    }

    // Filters: protocol, port ranges, then listening-only.
    rows.retain(|r| match opts.protocol {
        Protocol::Any => true,
        Protocol::Tcp => r.proto == "tcp",
        Protocol::Udp => r.proto == "udp",
    });
    if !ranges.is_empty() {
        rows.retain(|r| match r.port_num {
            Some(p) => ranges.iter().any(|(lo, hi)| p >= *lo && p <= *hi),
            None => false,
        });
    }
    let needle = opts.process.trim().to_ascii_lowercase();
    if !needle.is_empty() {
        rows.retain(|r| r.process.to_ascii_lowercase().contains(&needle));
    }
    if opts.listening_only {
        rows.retain(|r| r.is_listening());
    }

    let conflicts = find_conflicts(&rows);
    if opts.conflicts_only {
        rows.retain(|r| {
            conflicts
                .iter()
                .any(|c| c.proto == r.proto && c.port == r.port)
        });
    }

    sort_rows(&mut rows, opts.sort_by);

    let conflict_key = |r: &SocketRow| {
        conflicts
            .iter()
            .any(|c| c.proto == r.proto && c.port == r.port)
    };

    Ok(match opts.output_format {
        OutputFormat::Markdown => {
            render_markdown(&rows, &conflicts, detected, &opts, &conflict_key)
        }
        OutputFormat::Text => render_text(&rows, &conflicts, detected, &opts, &conflict_key),
        OutputFormat::Csv => render_csv(&rows, &opts, &conflict_key),
        OutputFormat::Json => render_json(&rows, &conflicts, detected, &opts, &conflict_key),
    })
}

// ---------------------------------------------------------------- detection

fn detect(input: &str) -> Option<InputFormat> {
    let (mut lsof, mut ss, mut netstat, mut win) = (0usize, 0usize, 0usize, 0usize);
    for line in input.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if line.contains("users:((") || t[0] == "Netid" {
            ss += 4;
            continue;
        }
        if t[0] == "COMMAND" && t.contains(&"PID") {
            lsof += 4;
            continue;
        }
        if matches!(t[0], "TCP" | "UDP" | "TCPv6" | "UDPv6") && t.len() >= 3 {
            win += 1;
            continue;
        }
        let base = proto_base(t[0]);
        if base.is_some() {
            if t.len() >= 6 && is_digits(t[1]) && is_digits(t[2]) {
                netstat += 1;
            } else if t.len() >= 5 && is_state_word(t[1]) {
                ss += 1;
            }
            continue;
        }
        if t.len() >= 8
            && is_digits(t[1])
            && t.iter()
                .skip(3)
                .any(|w| matches!(*w, "TCP" | "UDP" | "IPv4" | "IPv6"))
        {
            lsof += 1;
        }
    }
    let best = [
        (ss, InputFormat::Ss),
        (lsof, InputFormat::Lsof),
        (netstat, InputFormat::Netstat),
        (win, InputFormat::NetstatWindows),
    ]
    .into_iter()
    .max_by_key(|(n, _)| *n)?;
    if best.0 == 0 {
        None
    } else {
        Some(best.1)
    }
}

fn proto_base(tok: &str) -> Option<&'static str> {
    match tok.to_ascii_lowercase().as_str() {
        "tcp" | "tcp4" | "tcp6" | "tcpv6" => Some("tcp"),
        "udp" | "udp4" | "udp6" | "udpv6" | "udplite" | "udplite6" => Some("udp"),
        "raw" | "raw4" | "raw6" => Some("raw"),
        "sctp" | "sctp6" => Some("sctp"),
        _ => None,
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_state_word(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b == b'-' || b == b'_')
}

// ---------------------------------------------------------------- parsers

fn normalise_state(raw: &str) -> String {
    match raw.trim().to_ascii_uppercase().as_str() {
        "LISTENING" | "LISTEN" => "LISTEN".to_string(),
        "ESTAB" | "ESTABLISHED" => "ESTABLISHED".to_string(),
        "" | "-" => String::new(),
        other => other.to_string(),
    }
}

/// Split `host:port` (also `[::1]:631`, `*:*`, `:::80`) into its two halves.
fn split_host_port(addr: &str) -> (String, String) {
    let a = addr.trim();
    if let Some(close) = a.rfind(']') {
        if a.starts_with('[') {
            let host = a[1..close].to_string();
            let port = a[close + 1..].trim_start_matches(':').to_string();
            return (host, port);
        }
    }
    match a.rfind(':') {
        Some(i) => (a[..i].to_string(), a[i + 1..].to_string()),
        None => (a.to_string(), String::new()),
    }
}

fn looks_ipv6(host: &str) -> bool {
    host.contains(':')
}

/// A remote endpoint that carries no information (wildcards printed by every dialect).
fn peer_is_empty(peer: &str) -> bool {
    matches!(
        peer.trim(),
        "" | "*:*" | "0.0.0.0:*" | "0.0.0.0:0" | ":::*" | "[::]:*" | "[::]:0" | "*" | "-"
    )
}

fn port_num(port: &str) -> Option<u32> {
    port.parse::<u32>().ok().filter(|p| *p <= 65_535)
}

fn mk_row(
    proto: &str,
    ipv6_hint: bool,
    local: &str,
    peer: &str,
    state: &str,
    pid: Option<u32>,
    process: &str,
    user: &str,
) -> SocketRow {
    let (host, port) = split_host_port(local);
    let host = if host.is_empty() {
        "*".to_string()
    } else {
        host
    };
    SocketRow {
        proto: proto.to_string(),
        ipv6: ipv6_hint || looks_ipv6(&host),
        port_num: port_num(&port),
        address: host,
        port: if port.is_empty() { "*".into() } else { port },
        state: normalise_state(state),
        pid,
        process: if process.trim().is_empty() {
            "-".into()
        } else {
            process.trim().to_string()
        },
        user: if user.trim().is_empty() {
            "-".into()
        } else {
            user.trim().to_string()
        },
        peer: if peer_is_empty(peer) {
            String::new()
        } else {
            peer.trim().to_string()
        },
    }
}

/// `lsof -i` / `lsof -i -P -n`:
/// `COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME`
fn parse_lsof(input: &str) -> Vec<SocketRow> {
    let mut out = Vec::new();
    for line in input.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 4 || t[0] == "COMMAND" {
            continue;
        }
        // COMMAND may contain spaces — the PID is the first all-digit token after it.
        let pid_at = match t.iter().skip(1).position(|w| is_digits(w)).map(|i| i + 1) {
            Some(i) => i,
            None => continue,
        };
        let command = t[..pid_at].join(" ");
        let pid = t[pid_at].parse::<u32>().ok();
        let user = t.get(pid_at + 1).copied().unwrap_or("-");
        // NODE is the TCP/UDP token; everything after it is NAME.
        let node_at = match t
            .iter()
            .enumerate()
            .skip(pid_at + 2)
            .find(|(_, w)| proto_base(w).is_some())
            .map(|(i, _)| i)
        {
            Some(i) => i,
            None => continue,
        };
        let proto = proto_base(t[node_at]).unwrap_or("tcp");
        let ipv6 = t[pid_at + 1..node_at]
            .iter()
            .any(|w| w.eq_ignore_ascii_case("IPv6"));
        let name = t[node_at + 1..].join(" ");
        if name.is_empty() {
            continue;
        }
        // `10.0.0.5:52344->93.184.216.34:443 (ESTABLISHED)`
        let (endpoints, state) = match name.split_once(" (") {
            Some((e, s)) => (e, s.trim_end_matches(')')),
            None => (name.as_str(), ""),
        };
        let (local, peer) = match endpoints.split_once("->") {
            Some((l, p)) => (l, p),
            None => (endpoints, ""),
        };
        out.push(mk_row(proto, ipv6, local, peer, state, pid, &command, user));
    }
    out
}

/// `ss -tulpn`: `Netid State Recv-Q Send-Q Local:Port Peer:Port Process`.
/// `ss -tlnp` drops the Netid column, so the state word leads the row instead.
fn parse_ss(input: &str) -> Vec<SocketRow> {
    let mut out = Vec::new();
    for line in input.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 5 || t[0] == "Netid" || t[0] == "State" {
            continue;
        }
        let (proto, state, rest) = match proto_base(t[0]) {
            Some(p) => (p.to_string(), t[1].to_string(), 2usize),
            None if is_state_word(t[0]) => {
                // No Netid column: infer the family from the state.
                let p = if t[0].eq_ignore_ascii_case("UNCONN") {
                    "udp"
                } else {
                    "tcp"
                };
                (p.to_string(), t[0].to_string(), 1usize)
            }
            None => continue,
        };
        // Recv-Q / Send-Q, then the two endpoints.
        if t.len() < rest + 4 {
            continue;
        }
        if !is_digits(t[rest]) || !is_digits(t[rest + 1]) {
            continue;
        }
        let local = t[rest + 2];
        let peer = t[rest + 3];
        let tail = t[rest + 4..].join(" ");
        let holders = parse_ss_users(&tail);
        if holders.is_empty() {
            out.push(mk_row(&proto, false, local, peer, &state, None, "-", "-"));
        } else {
            for (name, pid) in holders {
                out.push(mk_row(&proto, false, local, peer, &state, pid, &name, "-"));
            }
        }
    }
    out
}

/// Pull `("nginx",pid=1234,fd=6)` tuples out of an `ss` Process column.
fn parse_ss_users(tail: &str) -> Vec<(String, Option<u32>)> {
    let mut out = Vec::new();
    let bytes: Vec<char> = tail.chars().collect();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == '(' && bytes[i + 1] == '"' {
            let start = i + 2;
            let mut j = start;
            while j < bytes.len() && bytes[j] != '"' {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            let name: String = bytes[start..j].iter().collect();
            // Look ahead for pid=<digits> before the closing paren.
            let mut k = j;
            let mut pid = None;
            while k < bytes.len() && bytes[k] != ')' {
                if bytes[k..].starts_with(&['p', 'i', 'd', '=']) {
                    let mut d = k + 4;
                    let mut num = String::new();
                    while d < bytes.len() && bytes[d].is_ascii_digit() {
                        num.push(bytes[d]);
                        d += 1;
                    }
                    pid = num.parse::<u32>().ok();
                    break;
                }
                k += 1;
            }
            out.push((name, pid));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// Linux `netstat -tulpn`:
/// `Proto Recv-Q Send-Q Local-Address Foreign-Address [State] PID/Program name`
fn parse_netstat(input: &str) -> Vec<SocketRow> {
    let mut out = Vec::new();
    for line in input.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 5 {
            continue;
        }
        let proto = match proto_base(t[0]) {
            Some(p) => p,
            None => continue,
        };
        if !is_digits(t[1]) || !is_digits(t[2]) {
            continue;
        }
        let ipv6 = t[0].to_ascii_lowercase().ends_with('6');
        let local = t[3];
        let peer = t[4];
        let mut idx = 5;
        let mut state = "";
        if let Some(tok) = t.get(idx) {
            // UDP rows print no State column; the PID/Program field holds a `/`.
            if is_state_word(tok) && !tok.contains('/') {
                state = tok;
                idx += 1;
            }
        }
        let prog = t[idx.min(t.len())..].join(" ");
        let (pid, name) = split_pid_program(&prog);
        out.push(mk_row(proto, ipv6, local, peer, state, pid, &name, "-"));
    }
    out
}

/// `575/sshd` → `(Some(575), "sshd")`; `-` → `(None, "-")`.
fn split_pid_program(field: &str) -> (Option<u32>, String) {
    let f = field.trim();
    if f.is_empty() || f == "-" {
        return (None, "-".to_string());
    }
    match f.split_once('/') {
        Some((p, name)) if is_digits(p) => (p.parse().ok(), name.trim().to_string()),
        _ => (None, f.to_string()),
    }
}

/// Windows `netstat -ano`: `Proto Local Foreign [State] PID`.
/// `netstat -anb` adds an `[image.exe]` line under each row — it is folded in.
fn parse_netstat_windows(input: &str) -> Vec<SocketRow> {
    let mut out: Vec<SocketRow> = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() > 2 {
            if let Some(last) = out.last_mut() {
                if last.process == "-" {
                    last.process = trimmed[1..trimmed.len() - 1].to_string();
                }
            }
            continue;
        }
        let t: Vec<&str> = trimmed.split_whitespace().collect();
        if t.len() < 3 {
            continue;
        }
        let proto = match proto_base(t[0]) {
            Some(p) if t[0].chars().all(|c| c.is_ascii_uppercase() || c == '6') => p,
            _ => continue,
        };
        let ipv6 = t[0].to_ascii_lowercase().ends_with("v6");
        let local = t[1];
        let peer = t[2];
        let (state, pid_tok) = if t.len() >= 5 {
            (t[3], t[4])
        } else if t.len() == 4 {
            if is_digits(t[3]) {
                ("", t[3])
            } else {
                (t[3], "")
            }
        } else {
            ("", "")
        };
        let pid = pid_tok.parse::<u32>().ok();
        out.push(mk_row(proto, ipv6, local, peer, state, pid, "-", "-"));
    }
    out
}

// ---------------------------------------------------------------- filters

fn parse_port_filter(spec: &str) -> Result<Vec<(u32, u32)>, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let (lo, hi) = match p.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (p, p),
        };
        let lo: u32 = lo.parse().map_err(|_| bad_ports(spec))?;
        let hi: u32 = hi.parse().map_err(|_| bad_ports(spec))?;
        if lo > 65_535 || hi > 65_535 {
            return Err(format!(
                "invalid ports filter \"{spec}\": port numbers must be 0-65535"
            ));
        }
        if lo > hi {
            return Err(format!(
                "invalid ports filter \"{spec}\": range {lo}-{hi} starts above where it ends"
            ));
        }
        out.push((lo, hi));
    }
    if out.is_empty() {
        return Err(bad_ports(spec));
    }
    Ok(out)
}

fn bad_ports(spec: &str) -> String {
    format!(
        "invalid ports filter \"{spec}\": expected comma-separated ports or ranges, \
         e.g. 80,443,8000-8100"
    )
}

/// A port is contended when more than one distinct PROGRAM listens on it. Several PIDs of
/// the same program on one port is a worker pool (nginx master + workers) and a program
/// bound to both the IPv4 and IPv6 wildcard is dual-stack — neither is a conflict. When the
/// source prints no program names at all (Windows `netstat -ano`), distinct PIDs are used
/// as the signal instead.
fn find_conflicts(rows: &[SocketRow]) -> Vec<Conflict> {
    let mut groups: Vec<(String, String, Vec<(Option<u32>, String, String)>)> = Vec::new();
    for r in rows.iter().filter(|r| r.is_listening()) {
        let entry = (r.pid, r.process.clone(), r.address.clone());
        match groups
            .iter_mut()
            .find(|(p, port, _)| *p == r.proto && *port == r.port)
        {
            Some((_, _, holders)) => {
                if !holders.contains(&entry) {
                    holders.push(entry);
                }
            }
            None => groups.push((r.proto.clone(), r.port.clone(), vec![entry])),
        }
    }
    groups
        .into_iter()
        .filter(|(_, _, holders)| {
            let mut names: Vec<&str> = holders
                .iter()
                .map(|h| h.1.as_str())
                .filter(|n| *n != "-")
                .collect();
            names.sort_unstable();
            names.dedup();
            if names.len() > 1 {
                return true;
            }
            let mut pids: Vec<u32> = holders.iter().filter_map(|h| h.0).collect();
            pids.sort_unstable();
            pids.dedup();
            names.is_empty() && pids.len() > 1
        })
        .map(|(proto, port, holders)| Conflict {
            proto,
            port,
            holders,
        })
        .collect()
}

fn sort_rows(rows: &mut [SocketRow], by: SortBy) {
    let port_key = |r: &SocketRow| (r.port_num.unwrap_or(u32::MAX), r.port.clone());
    match by {
        SortBy::Port => rows.sort_by(|a, b| {
            port_key(a)
                .cmp(&port_key(b))
                .then(a.proto_display().cmp(&b.proto_display()))
                .then(a.pid.cmp(&b.pid))
        }),
        SortBy::Pid => rows.sort_by(|a, b| {
            a.pid
                .unwrap_or(u32::MAX)
                .cmp(&b.pid.unwrap_or(u32::MAX))
                .then(port_key(a).cmp(&port_key(b)))
        }),
        SortBy::Process => rows.sort_by(|a, b| {
            a.process
                .to_ascii_lowercase()
                .cmp(&b.process.to_ascii_lowercase())
                .then(port_key(a).cmp(&port_key(b)))
        }),
        SortBy::State => rows.sort_by(|a, b| a.state.cmp(&b.state).then(port_key(a).cmp(&port_key(b)))),
        SortBy::Address => {
            rows.sort_by(|a, b| a.address.cmp(&b.address).then(port_key(a).cmp(&port_key(b))))
        }
    }
}

// ---------------------------------------------------------------- rendering

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn summary_line(rows: &[SocketRow], conflicts: &[Conflict], detected: InputFormat) -> String {
    let listening = rows.iter().filter(|r| r.is_listening()).count();
    let mut ports: Vec<&str> = rows.iter().map(|r| r.port.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    format!(
        "{}, {} listening, {}, {}, parsed as {}",
        plural(rows.len(), "row", "rows"),
        listening,
        plural(ports.len(), "unique port", "unique ports"),
        plural(conflicts.len(), "conflict", "conflicts"),
        detected.label()
    )
}

fn conflict_lines(conflicts: &[Conflict]) -> Vec<String> {
    conflicts
        .iter()
        .map(|c| {
            let who: Vec<String> = c
                .holders
                .iter()
                .map(|(pid, name, addr)| match (pid, name.as_str()) {
                    (Some(p), "-") => format!("PID {p} on {addr}"),
                    (Some(p), n) => format!("{n} (PID {p}) on {addr}"),
                    (None, "-") => format!("an unknown process on {addr}"),
                    (None, n) => format!("{n} (PID unknown) on {addr}"),
                })
                .collect();
            format!(
                "{} port {} is bound by {}: {}",
                c.proto,
                c.port,
                plural(c.holders.len(), "process", "processes"),
                who.join(", ")
            )
        })
        .collect()
}

fn peer_column_needed(rows: &[SocketRow]) -> bool {
    rows.iter().any(|r| !r.peer.is_empty())
}

/// One ready-to-run command line per distinct proto+port that has at least one
/// known PID: the Linux/macOS `kill` form and the Windows `taskkill` form, both
/// pre-filled with every PID holding that port. Truncated at
/// [`MAX_KILL_SUGGESTIONS`] ports with a trailing note.
fn kill_suggestions(rows: &[SocketRow]) -> Vec<String> {
    let mut groups: Vec<(String, String, Vec<(u32, String)>)> = Vec::new();
    for r in rows {
        let pid = match r.pid {
            Some(p) => p,
            None => continue,
        };
        let entry = (pid, r.process.clone());
        match groups
            .iter_mut()
            .find(|(proto, port, _)| *proto == r.proto && *port == r.port)
        {
            Some((_, _, holders)) => {
                if !holders.iter().any(|h| h.0 == pid) {
                    holders.push(entry);
                }
            }
            None => groups.push((r.proto.clone(), r.port.clone(), vec![entry])),
        }
    }
    let total = groups.len();
    let mut out: Vec<String> = groups
        .iter()
        .take(MAX_KILL_SUGGESTIONS)
        .map(|(proto, port, holders)| {
            let who: Vec<String> = holders
                .iter()
                .map(|(pid, name)| {
                    if name == "-" {
                        format!("PID {pid}")
                    } else {
                        format!("{name} PID {pid}")
                    }
                })
                .collect();
            let pids: Vec<String> = holders.iter().map(|(p, _)| p.to_string()).collect();
            let taskkill: Vec<String> = pids.iter().map(|p| format!("/PID {p}")).collect();
            format!(
                "{proto} {port} ({}) — Linux/macOS: kill -9 {} · Windows: taskkill {} /F",
                who.join(", "),
                pids.join(" "),
                taskkill.join(" ")
            )
        })
        .collect();
    if total > MAX_KILL_SUGGESTIONS {
        out.push(format!(
            "… and {} more port(s); narrow the table with the ports or process filter",
            total - MAX_KILL_SUGGESTIONS
        ));
    }
    out
}

fn empty_note() -> &'static str {
    "No sockets matched the current filters. Try turning off \"Listening sockets only\", \
     widening the protocol, or clearing the port filter."
}

/// Header labels + per-row cells, shared by the markdown and text tables so the
/// two can never disagree about which columns are present.
fn table_columns(
    rows: &[SocketRow],
    conflicts: &[Conflict],
    opts: &Options,
    is_conflict: &dyn Fn(&SocketRow) -> bool,
    upper: bool,
    blank: &str,
) -> (Vec<String>, Vec<Vec<String>>) {
    let peer = peer_column_needed(rows);
    let flag = !conflicts.is_empty();
    let mut head: Vec<String> = vec!["Proto", "Address", "Port"]
        .into_iter()
        .map(String::from)
        .collect();
    if opts.annotate_services {
        head.push("Service".into());
    }
    head.extend(
        ["State", "PID", "Process", "User"]
            .into_iter()
            .map(String::from),
    );
    if peer {
        head.push("Peer".into());
    }
    if flag {
        head.push("Conflict".into());
    }
    if upper {
        head = head.iter().map(|h| h.to_ascii_uppercase()).collect();
    }
    let body = rows
        .iter()
        .map(|r| {
            let mut cells = vec![r.proto_display(), r.address.clone(), r.port.clone()];
            if opts.annotate_services {
                cells.push(or_dash(r.service()));
            }
            cells.push(or_dash(&r.state));
            cells.push(r.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()));
            cells.push(r.process.clone());
            cells.push(r.user.clone());
            if peer {
                cells.push(or_dash(&r.peer));
            }
            if flag {
                cells.push(if is_conflict(r) {
                    "yes".into()
                } else {
                    blank.into()
                });
            }
            cells
        })
        .collect();
    (head, body)
}

fn or_dash(s: &str) -> String {
    if s.is_empty() {
        "-".to_string()
    } else {
        s.to_string()
    }
}

fn render_markdown(
    rows: &[SocketRow],
    conflicts: &[Conflict],
    detected: InputFormat,
    opts: &Options,
    is_conflict: &dyn Fn(&SocketRow) -> bool,
) -> String {
    if rows.is_empty() {
        return empty_note().to_string();
    }
    let (head, body) = table_columns(rows, conflicts, opts, is_conflict, false, "no");
    let mut out = String::new();
    out.push_str(&format!("| {} |\n", head.join(" | ")));
    out.push_str(&format!(
        "| {} |\n",
        head.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));
    for cells in &body {
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.push_str(&format!(
        "\n**Summary:** {}\n",
        summary_line(rows, conflicts, detected)
    ));
    if !conflicts.is_empty() {
        out.push_str("\n## Port conflicts\n\n");
        for line in conflict_lines(conflicts) {
            out.push_str(&format!("- {line}\n"));
        }
    }
    if opts.kill_commands {
        let cmds = kill_suggestions(rows);
        if !cmds.is_empty() {
            out.push_str("\n## Free a port\n\n");
            for line in cmds {
                out.push_str(&format!("- {line}\n"));
            }
        }
    }
    out.trim_end().to_string()
}

fn render_text(
    rows: &[SocketRow],
    conflicts: &[Conflict],
    detected: InputFormat,
    opts: &Options,
    is_conflict: &dyn Fn(&SocketRow) -> bool,
) -> String {
    if rows.is_empty() {
        return empty_note().to_string();
    }
    // Same columns as the markdown table (table_columns is the single source),
    // just upper-cased headers and space padding instead of pipes.
    let (head, body) = table_columns(rows, conflicts, opts, is_conflict, true, "-");
    let mut table: Vec<Vec<String>> = vec![head];
    table.extend(body);
    let cols = table[0].len();
    let mut widths = vec![0usize; cols];
    for row in &table {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    for row in &table {
        let line: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, c)| format!("{:width$}", c, width = widths[i]))
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    }
    out.push_str(&format!(
        "\nSummary: {}\n",
        summary_line(rows, conflicts, detected)
    ));
    if !conflicts.is_empty() {
        out.push_str("\nPort conflicts:\n");
        for line in conflict_lines(conflicts) {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if opts.kill_commands {
        let cmds = kill_suggestions(rows);
        if !cmds.is_empty() {
            out.push_str("\nFree a port:\n");
            for line in cmds {
                out.push_str(&format!("  {line}\n"));
            }
        }
    }
    out.trim_end().to_string()
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(
    rows: &[SocketRow],
    opts: &Options,
    is_conflict: &dyn Fn(&SocketRow) -> bool,
) -> String {
    let mut out = String::from("proto,address,port,");
    if opts.annotate_services {
        out.push_str("service,");
    }
    out.push_str("state,pid,process,user,peer,conflict\n");
    for r in rows {
        let mut cells = vec![r.proto_display(), r.address.clone(), r.port.clone()];
        if opts.annotate_services {
            cells.push(r.service().to_string());
        }
        cells.extend([
            r.state.clone(),
            r.pid.map(|p| p.to_string()).unwrap_or_default(),
            r.process.clone(),
            r.user.clone(),
            r.peer.clone(),
            if is_conflict(r) { "yes".into() } else { "no".into() },
        ]);
        let line: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_json(
    rows: &[SocketRow],
    conflicts: &[Conflict],
    detected: InputFormat,
    opts: &Options,
    is_conflict: &dyn Fn(&SocketRow) -> bool,
) -> String {
    let mut out = String::from("{\n  \"rows\": [\n");
    for (i, r) in rows.iter().enumerate() {
        out.push_str("    {");
        out.push_str(&format!("\"proto\": {}", json_str(&r.proto_display())));
        out.push_str(&format!(", \"address\": {}", json_str(&r.address)));
        out.push_str(&format!(", \"port\": {}", json_str(&r.port)));
        out.push_str(&match r.port_num {
            Some(p) => format!(", \"port_number\": {p}"),
            None => ", \"port_number\": null".to_string(),
        });
        if opts.annotate_services {
            out.push_str(&format!(", \"service\": {}", json_str(r.service())));
        }
        out.push_str(&format!(", \"state\": {}", json_str(&r.state)));
        out.push_str(&match r.pid {
            Some(p) => format!(", \"pid\": {p}"),
            None => ", \"pid\": null".to_string(),
        });
        out.push_str(&format!(", \"process\": {}", json_str(&r.process)));
        out.push_str(&format!(", \"user\": {}", json_str(&r.user)));
        out.push_str(&format!(", \"peer\": {}", json_str(&r.peer)));
        out.push_str(&format!(", \"listening\": {}", r.is_listening()));
        out.push_str(&format!(", \"conflict\": {}", is_conflict(r)));
        out.push('}');
        if i + 1 < rows.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ],\n  \"conflicts\": [\n");
    for (i, c) in conflicts.iter().enumerate() {
        out.push_str(&format!(
            "    {{\"proto\": {}, \"port\": {}, \"holders\": [",
            json_str(&c.proto),
            json_str(&c.port)
        ));
        let holders: Vec<String> = c
            .holders
            .iter()
            .map(|(pid, name, addr)| {
                format!(
                    "{{\"pid\": {}, \"process\": {}, \"address\": {}}}",
                    pid.map(|p| p.to_string()).unwrap_or_else(|| "null".into()),
                    json_str(name),
                    json_str(addr)
                )
            })
            .collect();
        out.push_str(&holders.join(", "));
        out.push_str("]}");
        if i + 1 < conflicts.len() {
            out.push(',');
        }
        out.push('\n');
    }
    let listening = rows.iter().filter(|r| r.is_listening()).count();
    let mut ports: Vec<&str> = rows.iter().map(|r| r.port.as_str()).collect();
    ports.sort_unstable();
    ports.dedup();
    out.push_str("  ],\n  \"summary\": {");
    out.push_str(&format!("\"rows\": {}", rows.len()));
    out.push_str(&format!(", \"listening\": {listening}"));
    out.push_str(&format!(", \"unique_ports\": {}", ports.len()));
    out.push_str(&format!(", \"conflicts\": {}", conflicts.len()));
    out.push_str(&format!(
        ", \"detected_format\": {}",
        json_str(detected.label())
    ));
    out.push_str("}\n}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SS: &str = "Netid State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process\n\
tcp   LISTEN 0      128          0.0.0.0:22        0.0.0.0:*    users:((\"sshd\",pid=575,fd=3))\n\
tcp   LISTEN 0      511        127.0.0.1:8080      0.0.0.0:*    users:((\"nginx\",pid=1234,fd=6))\n\
tcp   LISTEN 0      511          0.0.0.0:8080      0.0.0.0:*    users:((\"node\",pid=4321,fd=20))\n\
udp   UNCONN 0      0            0.0.0.0:68        0.0.0.0:*    users:((\"dhclient\",pid=812,fd=6))";

    const LSOF: &str = "COMMAND  PID USER   FD   TYPE DEVICE SIZE/OFF NODE NAME\n\
sshd     575 root    3u  IPv4  19373      0t0  TCP *:22 (LISTEN)\n\
nginx   1234 www     6u  IPv4  19999      0t0  TCP 127.0.0.1:8080 (LISTEN)\n\
curl     999 joris   5u  IPv4 221142      0t0  TCP 10.0.0.5:52344->93.184.216.34:443 (ESTABLISHED)\n\
cupsd    900 root    7u  IPv6   4242      0t0  TCP [::1]:631 (LISTEN)";

    const NETSTAT: &str = "Active Internet connections (only servers)\n\
Proto Recv-Q Send-Q Local Address           Foreign Address         State       PID/Program name\n\
tcp        0      0 0.0.0.0:22              0.0.0.0:*               LISTEN      575/sshd\n\
tcp6       0      0 :::80                   :::*                    LISTEN      1234/nginx: master\n\
udp        0      0 0.0.0.0:68              0.0.0.0:*                           812/dhclient";

    const WIN: &str = "Active Connections\n\n  Proto  Local Address          Foreign Address        State           PID\n\
  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       1234\n\
  TCP    [::]:445               [::]:0                 LISTENING       4\n\
  UDP    0.0.0.0:5353           *:*                                    2500";

    fn opts(out: OutputFormat) -> Options {
        Options {
            output_format: out,
            ..Options::default()
        }
    }

    #[test]
    fn ss_happy_path_markdown_table_and_conflict() {
        let got = parse(SS, opts(OutputFormat::Markdown)).unwrap();
        assert!(got.contains("| tcp | 0.0.0.0 | 22 | ssh | LISTEN | 575 | sshd | - | no |"), "{got}");
        assert!(got.contains(
            "| tcp | 127.0.0.1 | 8080 | http-alt (dev server/proxy) | LISTEN | 1234 | nginx | - | yes |"
        ));
        assert!(got.contains("**Summary:** 4 rows, 4 listening, 3 unique ports, 1 conflict, parsed as ss"));
        assert!(got.contains(
            "- tcp port 8080 is bound by 2 processes: nginx (PID 1234) on 127.0.0.1, node (PID 4321) on 0.0.0.0"
        ));
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = parse("   \n  ", Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn unrecognised_input_is_an_error() {
        let err = parse("hello world\nthis is not socket output", Options::default()).unwrap_err();
        assert!(err.contains("could not detect the input format"), "{err}");
    }

    #[test]
    fn detects_each_dialect() {
        assert_eq!(detect(SS), Some(InputFormat::Ss));
        assert_eq!(detect(LSOF), Some(InputFormat::Lsof));
        assert_eq!(detect(NETSTAT), Some(InputFormat::Netstat));
        assert_eq!(detect(WIN), Some(InputFormat::NetstatWindows));
    }

    #[test]
    fn lsof_rows_keep_user_ipv6_and_peer() {
        let o = Options {
            listening_only: false,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(LSOF, o).unwrap();
        assert!(got.contains("tcp,*,22,ssh,LISTEN,575,sshd,root,,no"), "{got}");
        assert!(
            got.contains("tcp6,::1,631,ipp (CUPS),LISTEN,900,cupsd,root,,no"),
            "{got}"
        );
        // 52344 is an ephemeral client port: no service label, peer preserved.
        assert!(
            got.contains("tcp,10.0.0.5,52344,,ESTABLISHED,999,curl,joris,93.184.216.34:443,no"),
            "{got}"
        );
    }

    #[test]
    fn netstat_linux_handles_missing_state_and_spaced_program() {
        let got = parse(NETSTAT, opts(OutputFormat::Csv)).unwrap();
        assert!(got.contains("tcp,0.0.0.0,22,ssh,LISTEN,575,sshd,-,,no"), "{got}");
        assert!(
            got.contains("tcp6,::,80,http,LISTEN,1234,nginx: master,-,,no"),
            "{got}"
        );
        assert!(
            got.contains("udp,0.0.0.0,68,dhcp-client,,812,dhclient,-,,no"),
            "{got}"
        );
    }

    #[test]
    fn windows_netstat_and_anb_image_names() {
        let with_names = "  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       1234\n [nginx.exe]\n  UDP    0.0.0.0:5353           *:*                                    2500\n [mDNSResponder.exe]";
        let got = parse(with_names, opts(OutputFormat::Csv)).unwrap();
        assert!(
            got.contains("tcp,0.0.0.0,8080,http-alt (dev server/proxy),LISTEN,1234,nginx.exe,-,,no"),
            "{got}"
        );
        assert!(
            got.contains("udp,0.0.0.0,5353,mdns (Bonjour),,2500,mDNSResponder.exe,-,,no"),
            "{got}"
        );
        // Plain -ano output still parses, just without image names.
        let plain = parse(WIN, opts(OutputFormat::Csv)).unwrap();
        assert!(
            plain.contains("tcp6,::,445,microsoft-ds (SMB),LISTEN,4,-,-,,no"),
            "{plain}"
        );
    }

    #[test]
    fn protocol_and_port_filters_apply() {
        let o = Options {
            protocol: Protocol::Udp,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert_eq!(got.lines().count(), 2, "{got}");
        assert!(
            got.contains("udp,0.0.0.0,68,dhcp-client,UNCONN,812,dhclient"),
            "{got}"
        );

        let o = Options {
            ports: "8000-8100".into(),
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert_eq!(got.lines().count(), 3, "{got}");
    }

    #[test]
    fn conflicts_only_keeps_just_the_contended_port() {
        let o = Options {
            conflicts_only: true,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert_eq!(got.lines().count(), 3, "{got}");
        assert!(
            got.contains(",8080,http-alt (dev server/proxy),LISTEN,1234,nginx,-,,yes"),
            "{got}"
        );
        assert!(
            got.contains(",8080,http-alt (dev server/proxy),LISTEN,4321,node,-,,yes"),
            "{got}"
        );
    }

    #[test]
    fn listening_only_drops_established_sockets() {
        let got = parse(LSOF, opts(OutputFormat::Csv)).unwrap();
        assert!(!got.contains("curl"), "{got}");
        let o = Options {
            listening_only: false,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        assert!(parse(LSOF, o).unwrap().contains("curl"));
    }

    #[test]
    fn ss_without_netid_column_infers_the_family() {
        let input = "State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process\n\
LISTEN 0      128    0.0.0.0:22         0.0.0.0:*          users:((\"sshd\",pid=575,fd=3))";
        let got = parse(input, opts(OutputFormat::Csv)).unwrap();
        assert!(got.contains("tcp,0.0.0.0,22,ssh,LISTEN,575,sshd"), "{got}");
    }

    #[test]
    fn ss_lists_every_worker_pid_but_a_worker_pool_is_not_a_conflict() {
        let input = "tcp   LISTEN 0 511 0.0.0.0:80 0.0.0.0:* users:((\"nginx\",pid=11,fd=6),(\"nginx\",pid=12,fd=6))";
        let got = parse(input, opts(OutputFormat::Csv)).unwrap();
        assert_eq!(got.lines().count(), 3, "{got}");
        assert!(got.contains(",80,http,LISTEN,11,nginx,-,,no"), "{got}");
        assert!(got.contains(",80,http,LISTEN,12,nginx,-,,no"), "{got}");
    }

    #[test]
    fn dual_stack_bind_by_one_pid_is_not_a_conflict() {
        let input = "tcp        0      0 0.0.0.0:80              0.0.0.0:*               LISTEN      1234/nginx\n\
tcp6       0      0 :::80                   :::*                    LISTEN      1234/nginx";
        let got = parse(input, opts(OutputFormat::Markdown)).unwrap();
        assert!(got.contains("0 conflicts"), "{got}");
        assert!(!got.contains("Conflict"), "{got}");
    }

    #[test]
    fn windows_ano_without_names_falls_back_to_distinct_pids() {
        let input = "  TCP    0.0.0.0:3000           0.0.0.0:0              LISTENING       1234\n\
  TCP    127.0.0.1:3000         0.0.0.0:0              LISTENING       5678";
        let got = parse(input, opts(OutputFormat::Markdown)).unwrap();
        assert!(got.contains("1 conflict"), "{got}");
        assert!(
            got.contains("- tcp port 3000 is bound by 2 processes: PID 1234 on 0.0.0.0, PID 5678 on 127.0.0.1"),
            "{got}"
        );
    }

    #[test]
    fn sorting_orders_ports_numerically_not_lexically() {
        let input = "tcp LISTEN 0 128 0.0.0.0:9 0.0.0.0:* users:((\"a\",pid=1,fd=1))\n\
tcp LISTEN 0 128 0.0.0.0:80 0.0.0.0:* users:((\"b\",pid=2,fd=1))\n\
tcp LISTEN 0 128 0.0.0.0:8 0.0.0.0:* users:((\"c\",pid=3,fd=1))";
        let got = parse(input, opts(OutputFormat::Csv)).unwrap();
        let ports: Vec<&str> = got.lines().skip(1).map(|l| l.split(',').nth(2).unwrap()).collect();
        assert_eq!(ports, vec!["8", "9", "80"]);

        let o = Options {
            sort_by: SortBy::Process,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(input, o).unwrap();
        // proto,address,port,service,state,pid,process,… — process is column 6.
        let names: Vec<&str> = got.lines().skip(1).map(|l| l.split(',').nth(6).unwrap()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn json_output_carries_summary_and_conflicts() {
        let got = parse(SS, opts(OutputFormat::Json)).unwrap();
        assert!(got.contains("\"detected_format\": \"ss\""), "{got}");
        assert!(got.contains("\"unique_ports\": 3"), "{got}");
        assert!(
            got.contains("\"proto\": \"tcp\", \"port\": \"8080\", \"holders\": ["),
            "{got}"
        );
        assert!(got.contains("\"pid\": 4321, \"process\": \"node\""), "{got}");
    }

    #[test]
    fn text_output_is_aligned_and_lists_conflicts() {
        let got = parse(SS, opts(OutputFormat::Text)).unwrap();
        assert!(
            got.starts_with(
                "PROTO  ADDRESS    PORT  SERVICE                      STATE   PID   PROCESS   USER  CONFLICT"
            ),
            "{got}"
        );
        assert!(got.contains("Port conflicts:"), "{got}");
    }

    #[test]
    fn service_annotation_can_be_turned_off() {
        let o = Options {
            annotate_services: false,
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert!(got.starts_with("proto,address,port,state,pid,process,user,peer,conflict"), "{got}");
        assert!(got.contains("tcp,0.0.0.0,22,LISTEN,575,sshd,-,,no"), "{got}");
    }

    #[test]
    fn process_filter_matches_a_case_insensitive_substring() {
        let o = Options {
            process: "NGIN".into(),
            output_format: OutputFormat::Csv,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert_eq!(got.lines().count(), 2, "{got}");
        assert!(got.contains(",nginx,"), "{got}");
        assert!(!got.contains(",node,"), "{got}");
    }

    #[test]
    fn kill_commands_list_every_pid_holding_a_port() {
        let o = Options {
            kill_commands: true,
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert!(got.contains("## Free a port"), "{got}");
        assert!(
            got.contains("tcp 22 (sshd PID 575) — Linux/macOS: kill -9 575 · Windows: taskkill /PID 575 /F"),
            "{got}"
        );
        // Both holders of the contended port land on one command line.
        assert!(
            got.contains("tcp 8080 (nginx PID 1234, node PID 4321) — Linux/macOS: kill -9 1234 4321 · Windows: taskkill /PID 1234 /PID 4321 /F"),
            "{got}"
        );
        // Off by default.
        assert!(!parse(SS, Options::default()).unwrap().contains("Free a port"));
    }

    #[test]
    fn forced_format_that_does_not_match_reports_it() {
        let o = Options {
            input_format: InputFormat::Lsof,
            ..Options::default()
        };
        let err = parse(NETSTAT, o).unwrap_err();
        assert!(err.contains("no socket rows found"), "{err}");
    }

    #[test]
    fn bad_port_filter_is_rejected() {
        let o = Options {
            ports: "80,http".into(),
            ..Options::default()
        };
        let err = parse(SS, o).unwrap_err();
        assert!(err.contains("comma-separated ports or ranges"), "{err}");

        let o = Options {
            ports: "9000-80".into(),
            ..Options::default()
        };
        let err = parse(SS, o).unwrap_err();
        assert!(err.contains("starts above where it ends"), "{err}");
    }

    #[test]
    fn line_cap_boundary() {
        let row = "tcp LISTEN 0 128 0.0.0.0:22 0.0.0.0:* users:((\"sshd\",pid=575,fd=3))";
        let at_cap = vec![row; MAX_LINES].join("\n");
        assert!(parse(&at_cap, opts(OutputFormat::Csv)).is_ok());
        let over_cap = vec![row; MAX_LINES + 1].join("\n");
        let err = parse(&over_cap, opts(OutputFormat::Csv)).unwrap_err();
        assert!(err.contains("over the 20000-line limit"), "{err}");
    }

    #[test]
    fn no_rows_after_filtering_says_so() {
        let o = Options {
            ports: "9999".into(),
            ..Options::default()
        };
        let got = parse(SS, o).unwrap();
        assert!(got.starts_with("No sockets matched the current filters."), "{got}");
    }
}
