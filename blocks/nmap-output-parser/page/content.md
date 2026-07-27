## About this tool

**Nmap Output Parser** converts raw nmap scan output into a table you can sort, copy, diff or feed into a report. It understands the two nmap formats most often saved from automated scans:

- XML from `nmap -oX scan.xml`
- Greppable text from `nmap -oG scan.gnmap`

The parser extracts the host, hostname, port, protocol, state, service name and detected version string. By default it keeps only open ports so the result is report-ready, but you can turn **Open ports only** off to include closed and filtered rows too.

### Worked example

Input:

```xml
<nmaprun>
  <host>
    <address addr="10.0.0.10" addrtype="ipv4"/>
    <hostnames><hostname name="web.local"/></hostnames>
    <ports>
      <port protocol="tcp" portid="80">
        <state state="open"/>
        <service name="http" product="nginx" version="1.18.0"/>
      </port>
    </ports>
  </host>
</nmaprun>
```

Output as Markdown:

```markdown
| Host       | Hostname  | Port | Protocol | State | Service | Version      |
| ---------- | --------- | ---- | -------- | ----- | ------- | ------------ |
| 10.0.0.10 | web.local | 80   | tcp      | open  | http    | nginx 1.18.0 |
```

### Options

- **Input format** — auto-detect, XML (`-oX`) or greppable (`-oG`).
- **Output** — Markdown table, CSV or JSON.
- **Sort by** — host, port or service. IPv4 hosts sort numerically, so `10.0.0.9` comes before `10.0.0.10`.
- **Open ports only** — enabled by default; disable it to include closed or filtered ports.

## FAQ

<details>
<summary>Does this run nmap for me?</summary>

No. It parses output you already have. Run nmap separately and paste XML from `nmap -oX` or greppable output from `nmap -oG`.

</details>

<details>
<summary>Which nmap output format should I use?</summary>

XML (`-oX`) is the most structured and preserves service/version details reliably. Greppable (`-oG`) is convenient for older scripts and quick scans, and this tool can parse its `Host:` / `Ports:` records too.

</details>

<details>
<summary>Why did closed ports disappear?</summary>

**Open ports only** is on by default because most reports focus on reachable services. Turn it off to include `closed`, `filtered` and other states found in the scan.

</details>

<details>
<summary>Does it parse UDP and IPv6 scans?</summary>

Yes. Protocols such as `tcp` and `udp` are preserved from the nmap output, and IPv6 host strings are kept. IPv4 hosts get numeric sorting; IPv6 and names sort as strings.

</details>
