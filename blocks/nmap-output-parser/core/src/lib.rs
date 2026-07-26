//! nmap-output-parser core — parse nmap XML (`-oX`) or greppable (`-oG`) scan
//! output into a clean, sortable table of hosts, open ports, services and
//! versions, rendered as markdown, CSV or JSON. Pure-Rust (`quick-xml`), no
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// One host:port row of the parsed scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortRow {
    /// IP address of the host.
    pub host: String,
    /// Reverse-DNS / PTR hostname if the scan captured one, else empty.
    pub hostname: String,
    /// Port number.
    pub port: u16,
    /// Transport protocol (`tcp`, `udp`, `sctp`, `ip`).
    pub protocol: String,
    /// Port state (`open`, `closed`, `filtered`, `open|filtered`, …).
    pub state: String,
    /// Detected service name (`ssh`, `http`, …) or empty.
    pub service: String,
    /// Product + version + extra info joined into one human string, e.g.
    /// `OpenSSH 8.2p1 Ubuntu 4ubuntu0.5`. Empty when nmap reported nothing.
    pub version: String,
}

/// Which nmap output format the input is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// nmap `-oX` XML.
    Xml,
    /// nmap `-oG` greppable.
    Greppable,
    /// Sniff the format from the text.
    Auto,
}

impl InputFormat {
    /// Parse the `format` param. Blank/unknown falls back to auto-detect.
    pub fn parse(s: &str) -> InputFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "xml" | "-ox" => InputFormat::Xml,
            "greppable" | "grep" | "grepable" | "-og" => InputFormat::Greppable,
            _ => InputFormat::Auto,
        }
    }
}

/// Output table format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Markdown,
    Csv,
    Json,
}

impl OutputFormat {
    /// Parse the `output` param. Blank/unknown falls back to markdown.
    pub fn parse(s: &str) -> OutputFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "csv" => OutputFormat::Csv,
            "json" => OutputFormat::Json,
            _ => OutputFormat::Markdown,
        }
    }
}

/// Column to sort rows by (all sorts are ascending with stable tie-breaks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Host,
    Port,
    Service,
}

impl SortBy {
    /// Parse the `sort_by` param. Blank/unknown falls back to host.
    pub fn parse(s: &str) -> SortBy {
        match s.trim().to_ascii_lowercase().as_str() {
            "port" => SortBy::Port,
            "service" => SortBy::Service,
            _ => SortBy::Host,
        }
    }
}

/// Options controlling parsing + rendering.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub input_format: InputFormat,
    pub output_format: OutputFormat,
    pub sort_by: SortBy,
    /// Keep only `open` / `open|filtered` ports (default). When false, every
    /// port state that appears in the scan is included.
    pub open_only: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            output_format: OutputFormat::Markdown,
            sort_by: SortBy::Host,
            open_only: true,
        }
    }
}

/// Parse `input` per `opts` and render the sorted table. Returns a descriptive
/// error for empty/unrecognized input or malformed XML.
pub fn parse(input: &str, opts: Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("no nmap output provided".into());
    }

    let fmt = match opts.input_format {
        InputFormat::Auto => detect_format(input)?,
        other => other,
    };

    let mut rows = match fmt {
        InputFormat::Xml => parse_xml(input)?,
        InputFormat::Greppable => parse_greppable(input)?,
        InputFormat::Auto => unreachable!("auto resolved above"),
    };

    if opts.open_only {
        rows.retain(|r| r.state.starts_with("open"));
    }

    sort_rows(&mut rows, opts.sort_by);

    if rows.is_empty() {
        return Err(if opts.open_only {
            "parsed the scan but found no open ports (uncheck 'open only' to include closed/filtered ports)".into()
        } else {
            "parsed the scan but found no ports to report".into()
        });
    }

    Ok(render(&rows, opts.output_format))
}

/// Decide XML vs greppable from the text. XML starts (after whitespace) with
/// `<`; greppable output has `Host:` lines with `Ports:`/`Status:` fields.
fn detect_format(input: &str) -> Result<InputFormat, String> {
    let trimmed = input.trim_start();
    if trimmed.starts_with('<') {
        return Ok(InputFormat::Xml);
    }
    if input.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("Host:") && (l.contains("Ports:") || l.contains("Status:"))
    }) {
        return Ok(InputFormat::Greppable);
    }
    Err("could not detect the nmap output format — paste XML from `nmap -oX` or greppable output from `nmap -oG`, or set the format explicitly".into())
}

fn sort_rows(rows: &mut [PortRow], by: SortBy) {
    match by {
        SortBy::Host => rows.sort_by(|a, b| {
            host_key(&a.host)
                .cmp(&host_key(&b.host))
                .then(a.port.cmp(&b.port))
                .then(a.protocol.cmp(&b.protocol))
        }),
        SortBy::Port => rows.sort_by(|a, b| {
            a.port
                .cmp(&b.port)
                .then(a.protocol.cmp(&b.protocol))
                .then(host_key(&a.host).cmp(&host_key(&b.host)))
        }),
        SortBy::Service => rows.sort_by(|a, b| {
            a.service
                .to_ascii_lowercase()
                .cmp(&b.service.to_ascii_lowercase())
                .then(host_key(&a.host).cmp(&host_key(&b.host)))
                .then(a.port.cmp(&b.port))
        }),
    }
}

/// A sort key that orders IPv4 addresses numerically (so `.9` sorts before
/// `.10`) and falls back to the raw string for hostnames / IPv6.
fn host_key(host: &str) -> (u8, [u16; 4], String) {
    let octets: Vec<&str> = host.split('.').collect();
    if octets.len() == 4 {
        let mut parsed = [0u16; 4];
        let mut ok = true;
        for (i, o) in octets.iter().enumerate() {
            match o.parse::<u16>() {
                Ok(n) if n <= 255 => parsed[i] = n,
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            // 0 => IPv4 bucket, sorts before the string bucket.
            return (0, parsed, String::new());
        }
    }
    (1, [0; 4], host.to_string())
}

/// Join product / version / extrainfo into one display string.
fn join_version(product: &str, version: &str, extrainfo: &str) -> String {
    [product, version, extrainfo]
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// XML parsing (nmap -oX)
// ---------------------------------------------------------------------------

fn parse_xml(xml: &str) -> Result<Vec<PortRow>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut rows: Vec<PortRow> = Vec::new();
    let mut saw_nmaprun = false;

    // Current host context.
    let mut cur_host = String::new();
    let mut cur_hostname = String::new();
    // Current port context.
    let mut cur_port: Option<u16> = None;
    let mut cur_proto = String::new();
    let mut cur_state = String::new();
    let mut cur_service = String::new();
    let mut cur_version = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"nmaprun" => saw_nmaprun = true,
                    b"host" => {
                        cur_host.clear();
                        cur_hostname.clear();
                    }
                    b"address" => {
                        // Prefer the IPv4/IPv6 address over the MAC address.
                        let mut addr = String::new();
                        let mut addrtype = String::new();
                        for a in e.attributes().flatten() {
                            match a.key.local_name().as_ref() {
                                b"addr" => addr = attr_value(&a),
                                b"addrtype" => addrtype = attr_value(&a),
                                _ => {}
                            }
                        }
                        if addrtype != "mac" && !addr.is_empty() {
                            cur_host = addr;
                        }
                    }
                    b"hostname" => {
                        // First hostname wins (nmap lists PTR first).
                        if cur_hostname.is_empty() {
                            for a in e.attributes().flatten() {
                                if a.key.local_name().as_ref() == b"name" {
                                    cur_hostname = attr_value(&a);
                                }
                            }
                        }
                    }
                    b"port" => {
                        cur_port = None;
                        cur_proto.clear();
                        cur_state.clear();
                        cur_service.clear();
                        cur_version.clear();
                        for a in e.attributes().flatten() {
                            match a.key.local_name().as_ref() {
                                b"portid" => cur_port = attr_value(&a).parse().ok(),
                                b"protocol" => cur_proto = attr_value(&a),
                                _ => {}
                            }
                        }
                    }
                    b"state" => {
                        for a in e.attributes().flatten() {
                            if a.key.local_name().as_ref() == b"state" {
                                cur_state = attr_value(&a);
                            }
                        }
                    }
                    b"service" => {
                        let mut product = String::new();
                        let mut version = String::new();
                        let mut extrainfo = String::new();
                        for a in e.attributes().flatten() {
                            match a.key.local_name().as_ref() {
                                b"name" => cur_service = attr_value(&a),
                                b"product" => product = attr_value(&a),
                                b"version" => version = attr_value(&a),
                                b"extrainfo" => extrainfo = attr_value(&a),
                                _ => {}
                            }
                        }
                        cur_version = join_version(&product, &version, &extrainfo);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"port" {
                    if let Some(port) = cur_port {
                        rows.push(PortRow {
                            host: cur_host.clone(),
                            hostname: cur_hostname.clone(),
                            port,
                            protocol: cur_proto.clone(),
                            state: cur_state.clone(),
                            service: cur_service.clone(),
                            version: cur_version.clone(),
                        });
                    }
                    cur_port = None;
                }
            }
            Err(e) => {
                return Err(format!(
                    "nmap XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ));
            }
            _ => {}
        }
    }

    if !saw_nmaprun {
        return Err(
            "this XML has no <nmaprun> root — it doesn't look like nmap `-oX` output".into(),
        );
    }
    Ok(rows)
}

fn attr_value(a: &quick_xml::events::attributes::Attribute) -> String {
    a.unescape_value()
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| String::from_utf8_lossy(&a.value).into_owned())
}

// ---------------------------------------------------------------------------
// Greppable parsing (nmap -oG)
// ---------------------------------------------------------------------------

fn parse_greppable(text: &str) -> Result<Vec<PortRow>, String> {
    let mut rows = Vec::new();
    let mut saw_host_line = false;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !line.starts_with("Host:") {
            continue;
        }
        saw_host_line = true;

        // A greppable record is tab-separated `Field: value` chunks:
        //   Host: 10.0.0.1 (router.local)\tPorts: 22/open/tcp//ssh//OpenSSH 8.2/, ...
        let (host, hostname) = parse_greppable_host(line);

        // Find the "Ports:" field (everything after it up to the next tab-field
        // is the port list). Split on tab so trailing fields don't leak in.
        let Some(idx) = line.find("Ports:") else {
            continue; // e.g. a "Status: Up" only line — no ports to add.
        };
        let after = &line[idx + "Ports:".len()..];
        let ports_field = after.split('\t').next().unwrap_or(after);

        for spec in ports_field.split(',') {
            let spec = spec.trim().trim_end_matches('/');
            if spec.is_empty() {
                continue;
            }
            if let Some(row) = parse_greppable_port(spec, &host, &hostname) {
                rows.push(row);
            }
        }
    }

    if !saw_host_line {
        return Err(
            "no `Host:` records found — this doesn't look like nmap `-oG` greppable output".into(),
        );
    }
    Ok(rows)
}

/// Extract the IP and optional `(hostname)` from `Host: 10.0.0.1 (router.local)`.
fn parse_greppable_host(line: &str) -> (String, String) {
    let after = line["Host:".len()..].trim_start();
    // The host chunk runs up to the first tab.
    let chunk = after.split('\t').next().unwrap_or(after).trim();
    let host = chunk.split_whitespace().next().unwrap_or("").to_string();
    let hostname = match (chunk.find('('), chunk.find(')')) {
        (Some(o), Some(c)) if c > o + 1 => chunk[o + 1..c].trim().to_string(),
        _ => String::new(),
    };
    (host, hostname)
}

/// Parse one greppable port spec:
/// `port/state/protocol/owner/service/rpc_info/version/`.
fn parse_greppable_port(spec: &str, host: &str, hostname: &str) -> Option<PortRow> {
    let f: Vec<&str> = spec.split('/').collect();
    let port: u16 = f.first()?.trim().parse().ok()?;
    let state = f.get(1).map(|s| s.trim()).unwrap_or("").to_string();
    let protocol = f.get(2).map(|s| s.trim()).unwrap_or("").to_string();
    let service = f.get(4).map(|s| s.trim()).unwrap_or("").to_string();
    // Field 6 is the version/product string (greppable escapes '/' as '|').
    let version = f
        .get(6)
        .map(|s| s.trim().replace('|', "/"))
        .unwrap_or_default();
    Some(PortRow {
        host: host.to_string(),
        hostname: hostname.to_string(),
        port,
        protocol,
        state,
        service,
        version,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

const HEADERS: [&str; 7] = [
    "Host", "Hostname", "Port", "Protocol", "State", "Service", "Version",
];

fn cells(r: &PortRow) -> [String; 7] {
    [
        r.host.clone(),
        r.hostname.clone(),
        r.port.to_string(),
        r.protocol.clone(),
        r.state.clone(),
        r.service.clone(),
        r.version.clone(),
    ]
}

fn render(rows: &[PortRow], fmt: OutputFormat) -> String {
    match fmt {
        OutputFormat::Markdown => render_markdown(rows),
        OutputFormat::Csv => render_csv(rows),
        OutputFormat::Json => render_json(rows),
    }
}

fn render_markdown(rows: &[PortRow]) -> String {
    // Compute column widths for a clean, aligned table.
    let mut widths: [usize; 7] = HEADERS.map(|h| h.len());
    let all: Vec<[String; 7]> = rows.iter().map(cells).collect();
    for row in &all {
        for (i, c) in row.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }

    let pad = |s: &str, w: usize| -> String {
        let extra = w.saturating_sub(s.chars().count());
        format!("{s}{}", " ".repeat(extra))
    };

    let mut out = String::new();
    // Header.
    out.push_str("| ");
    out.push_str(
        &HEADERS
            .iter()
            .enumerate()
            .map(|(i, h)| pad(h, widths[i]))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n");
    // Separator.
    out.push_str("| ");
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    out.push_str(" |\n");
    // Rows.
    for row in &all {
        out.push_str("| ");
        out.push_str(
            &row.iter()
                .enumerate()
                .map(|(i, c)| pad(c, widths[i]))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    // Trim the trailing newline for stable output.
    out.pop();
    out
}

fn render_csv(rows: &[PortRow]) -> String {
    let mut out = String::new();
    out.push_str(&HEADERS.join(","));
    out.push('\n');
    for r in rows {
        let line = cells(r)
            .iter()
            .map(|c| csv_field(c))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    out.pop();
    out
}

/// Quote a CSV field when it contains a comma, quote or newline (RFC 4180).
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_json(rows: &[PortRow]) -> String {
    if rows.is_empty() {
        return "[]".into();
    }
    let mut out = String::from("[");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n  {");
        out.push_str(&format!("\"host\": {}", json_str(&r.host)));
        out.push_str(&format!(", \"hostname\": {}", json_str(&r.hostname)));
        out.push_str(&format!(", \"port\": {}", r.port));
        out.push_str(&format!(", \"protocol\": {}", json_str(&r.protocol)));
        out.push_str(&format!(", \"state\": {}", json_str(&r.state)));
        out.push_str(&format!(", \"service\": {}", json_str(&r.service)));
        out.push_str(&format!(", \"version\": {}", json_str(&r.version)));
        out.push('}');
    }
    out.push_str("\n]");
    out
}

/// Minimal JSON string escaping (no external serde dep in core).
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

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?>
<!DOCTYPE nmaprun>
<nmaprun scanner="nmap" args="nmap -sV 10.0.0.10">
  <host>
    <address addr="10.0.0.10" addrtype="ipv4"/>
    <address addr="00:11:22:33:44:55" addrtype="mac" vendor="Acme"/>
    <hostnames><hostname name="web.local" type="PTR"/></hostnames>
    <ports>
      <port protocol="tcp" portid="80">
        <state state="open" reason="syn-ack"/>
        <service name="http" product="nginx" version="1.18.0" extrainfo="Ubuntu"/>
      </port>
      <port protocol="tcp" portid="22">
        <state state="open" reason="syn-ack"/>
        <service name="ssh" product="OpenSSH" version="8.2p1"/>
      </port>
      <port protocol="tcp" portid="443">
        <state state="closed" reason="reset"/>
        <service name="https"/>
      </port>
    </ports>
  </host>
</nmaprun>"#;

    #[test]
    fn parses_xml_open_ports_sorted_by_port() {
        let opts = Options {
            output_format: OutputFormat::Json,
            sort_by: SortBy::Port,
            ..Default::default()
        };
        let out = parse(XML, opts).unwrap();
        // Only open ports (22, 80), sorted by port → 22 first.
        assert!(out.contains("\"port\": 22"));
        assert!(out.contains("\"port\": 80"));
        assert!(!out.contains("\"port\": 443")); // closed, filtered out
        let p22 = out.find("\"port\": 22").unwrap();
        let p80 = out.find("\"port\": 80").unwrap();
        assert!(p22 < p80);
        // MAC address must not become the host.
        assert!(out.contains("\"host\": \"10.0.0.10\""));
        assert!(!out.contains("00:11:22"));
        // Version joins product+version+extrainfo.
        assert!(out.contains("nginx 1.18.0 Ubuntu"));
        assert!(out.contains("\"hostname\": \"web.local\""));
    }

    #[test]
    fn xml_include_closed_when_open_only_off() {
        let opts = Options {
            output_format: OutputFormat::Csv,
            open_only: false,
            ..Default::default()
        };
        let out = parse(XML, opts).unwrap();
        assert!(out.contains("443"));
        assert!(out.contains("closed"));
        // CSV header present.
        assert!(out.starts_with("Host,Hostname,Port,Protocol,State,Service,Version"));
    }

    #[test]
    fn parses_greppable() {
        let grep = "# Nmap 7.80 scan initiated\n\
            Host: 10.0.0.9 (a.local)\tStatus: Up\n\
            Host: 10.0.0.9 (a.local)\tPorts: 22/open/tcp//ssh//OpenSSH 8.2p1/, 80/open/tcp//http//nginx 1.18.0/\tIgnored State: closed (995)\n\
            # Nmap done";
        let opts = Options {
            input_format: InputFormat::Greppable,
            output_format: OutputFormat::Markdown,
            ..Default::default()
        };
        let out = parse(grep, opts).unwrap();
        assert!(out.contains("| Host"));
        assert!(out.contains("10.0.0.9"));
        assert!(out.contains("a.local"));
        assert!(out.contains("ssh"));
        assert!(out.contains("OpenSSH 8.2p1"));
        assert!(out.contains("nginx 1.18.0"));
        // The "Ignored State" trailing field must not leak into a port row.
        assert!(!out.contains("Ignored"));
    }

    #[test]
    fn auto_detects_xml_and_greppable() {
        assert_eq!(detect_format(XML).unwrap(), InputFormat::Xml);
        let grep = "Host: 1.2.3.4 (h)\tPorts: 22/open/tcp//ssh///\n";
        assert_eq!(detect_format(grep).unwrap(), InputFormat::Greppable);
    }

    #[test]
    fn ipv4_hosts_sort_numerically() {
        let grep = "Host: 10.0.0.10 ()\tPorts: 80/open/tcp//http///\n\
                    Host: 10.0.0.9 ()\tPorts: 80/open/tcp//http///\n";
        let opts = Options {
            input_format: InputFormat::Greppable,
            output_format: OutputFormat::Csv,
            sort_by: SortBy::Host,
            ..Default::default()
        };
        let out = parse(grep, opts).unwrap();
        let p9 = out.find("10.0.0.9").unwrap();
        let p10 = out.find("10.0.0.10").unwrap();
        assert!(p9 < p10, ".9 must sort before .10 numerically");
    }

    #[test]
    fn errors_on_empty_input() {
        assert!(parse("   ", Options::default()).is_err());
    }

    #[test]
    fn errors_on_unrecognized_input() {
        let err = parse("this is just prose, not a scan", Options::default()).unwrap_err();
        assert!(err.contains("could not detect"));
    }

    #[test]
    fn errors_on_malformed_xml() {
        let bad = "<nmaprun><host></nmaprun>";
        let err = parse(
            bad,
            Options {
                input_format: InputFormat::Xml,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("not well-formed"));
    }

    #[test]
    fn errors_when_no_open_ports() {
        let grep = "Host: 1.2.3.4 ()\tPorts: 443/closed/tcp//https///\n";
        let opts = Options {
            input_format: InputFormat::Greppable,
            ..Default::default()
        };
        let err = parse(grep, opts).unwrap_err();
        assert!(err.contains("no open ports"));
    }

    #[test]
    fn csv_quotes_fields_with_commas() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("he said \"hi\""), "\"he said \"\"hi\"\"\"");
    }
}
