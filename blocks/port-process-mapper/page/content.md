## About this tool

`lsof`, `ss` and `netstat` all answer the same question — *which process is holding this
port?* — and all three print it differently, on different platforms, with the answer split
across columns that shift depending on the flags you used. This tool takes that raw paste
and normalises it into one table: protocol, local address, port, well-known service, socket
state, PID, process name and user, in a layout that is the same whichever command produced it.

It also does the part the commands don't: it groups the rows by protocol + port and flags
every port that **more than one distinct process** is bound to. That is usually the real
question behind `EADDRINUSE` / "address already in use" — a stale dev server still holding
`3000`, a second nginx worker set, or a container publishing a port the host already owns.
Two PIDs from the same worker pool on one port are not counted as a conflict; two different
programs on one port are.

Nothing is executed and nothing is uploaded. You run the capture command yourself, paste the
text, and the parsing happens in your browser via WebAssembly.

### Worked example

Paste this `ss -tulpn` capture:

```
Netid State  Recv-Q Send-Q Local Address:Port Peer Address:Port Process
tcp   LISTEN 0      128          0.0.0.0:22        0.0.0.0:*    users:(("sshd",pid=575,fd=3))
tcp   LISTEN 0      511        127.0.0.1:8080      0.0.0.0:*    users:(("nginx",pid=1234,fd=6))
tcp   LISTEN 0      511          0.0.0.0:8080      0.0.0.0:*    users:(("node",pid=4321,fd=20))
udp   UNCONN 0      0            0.0.0.0:68        0.0.0.0:*    users:(("dhclient",pid=812,fd=6))
```

With the defaults (auto-detect, markdown, sorted by port, listening only, services named)
you get:

| Proto | Address | Port | Service | State | PID | Process | User | Conflict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| tcp | 0.0.0.0 | 22 | ssh | LISTEN | 575 | sshd | - | no |
| udp | 0.0.0.0 | 68 | dhcp-client | UNCONN | 812 | dhclient | - | no |
| tcp | 127.0.0.1 | 8080 | http-alt (dev server/proxy) | LISTEN | 1234 | nginx | - | yes |
| tcp | 0.0.0.0 | 8080 | http-alt (dev server/proxy) | LISTEN | 4321 | node | - | yes |

**Summary:** 4 rows, 4 listening, 3 unique ports, 1 conflict, parsed as ss

Port 8080 is called out underneath: *tcp port 8080 is bound by 2 processes: nginx (PID 1234)
on 127.0.0.1, node (PID 4321) on 0.0.0.0*. Tick **Show commands to free a port** and the
same run adds `kill -9 1234 4321` (Linux/macOS) and `taskkill /PID 1234 /PID 4321 /F`
(Windows) for that port.

### Capturing the input

| Platform | Command | Notes |
| --- | --- | --- |
| Linux | `ss -tulpn` | Needs `sudo` to see PIDs of other users' processes |
| Linux | `sudo netstat -tulpn` | Older hosts without `ss` |
| macOS / Linux | `sudo lsof -i -P -n` | `-P -n` keeps ports and addresses numeric |
| Windows | `netstat -ano` | PIDs only; add `-b` (as Administrator) for image names |

Include the header line — it is what the auto-detector keys on. If you pasted a fragment
with no header and detection fails, pick the dialect explicitly in **Input format**.

### Limits and behaviour

- Up to **20,000 lines** of input; past that you are pasting a log file, not a socket listing.
- **Listening sockets only** is on by default: `LISTEN`, `UNCONN` and stateless UDP rows are
  kept, established/time-wait client connections are dropped. Turn it off to see everything.
- The port filter accepts numbers and inclusive ranges (`80,443,8000-8100`). A row whose port
  prints as a service name rather than a number (`lsof` without `-P`) is dropped when that
  filter is set.
- The **Free a port** list covers at most 20 distinct ports; narrow the table first if you hit it.
- Service names cover the IANA well-known registrations you actually meet on a host plus the
  unregistered dev-server ports (3000, 4200, 5173, 8000, 8080, 9229, …). Unknown ports show `-`.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. -->

<details>
<summary>Which commands' output does it understand?</summary>

Four dialects: `lsof -i` (COMMAND/PID/USER/NAME columns), `ss -tulpn` (Netid/State plus
`users:(("name",pid=N,fd=N))`), Linux `netstat -tulpn` (`PID/Program name`), and Windows
`netstat -ano` / `netstat -anb` (PID in the last column, image name on the following line for
`-b`). **Input format** is set to auto-detect by default and scores the paste against all
four; set it explicitly if a trimmed paste gets read as the wrong one.

</details>

<details>
<summary>Why does it say a port has a conflict when it's just one program?</summary>

A conflict means two **distinct PIDs running different programs** are bound to the same
protocol + port. Multi-process servers are excluded on purpose: an nginx master with four
workers, or a Node cluster, shares one listening socket across PIDs with the same command
name, and that is reported as several rows but zero conflicts. A dual-stack bind — the same
PID on `0.0.0.0:80` and `:::80` — is also not a conflict. What does count is `nginx` on
`127.0.0.1:8080` while `node` holds `0.0.0.0:8080`, which is exactly the case that makes one
of them fail to start.

</details>

<details>
<summary>Does anything get sent to a server, or any process killed?</summary>

No. The parser is compiled to WebAssembly and runs in your browser tab; the text you paste
never leaves your device, and no capture command is run for you. The **Show commands to free
a port** option only *prints* `kill` / `taskkill` command lines with the right PIDs filled
in — you copy and run them yourself, after checking that the PID is really the one you meant.

</details>

<details>
<summary>Can I get the table as CSV or JSON instead?</summary>

Yes — **Output format** switches between a markdown table, a space-aligned text table for a
terminal, CSV (one row per socket, with a `conflict` column) and JSON. The JSON form returns
`rows`, a `conflicts` array listing each contended port's holders, and a `summary` object with
row/listening/unique-port/conflict counts and the dialect that was detected, so you can pipe
it into a script or a report.

</details>

<details>
<summary>Why is the PID or process name a dash?</summary>

Because the capture didn't include it. `ss`, `netstat` and `lsof` only show the owning process
for sockets your user owns unless you run them with `sudo`; Windows `netstat -ano` gives PIDs
but no image names unless you run `netstat -anb` as Administrator. Re-capture with elevated
privileges and the column fills in. Rows with an unknown PID are still parsed, sorted and
conflict-checked — they just can't appear in the kill-command list.

</details>
