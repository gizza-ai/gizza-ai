# port-process-mapper — competitor analysis (2026-08-29)

Scan run while finishing the tool's surfaces. All notes below are paraphrased observations of
publicly documented behaviour; no competitor copy, branding, trademarks or assets are reproduced
or reused.

## Scan

Four web searches (paste-in parsers for `lsof`/`ss`/`netstat` output; netstat listening-port/PID
analysis; the Windows socket-viewer GUIs; port-number → service lookup sites). The finding that
shapes everything below: **there is essentially no direct competitor**. Nobody ships a browser tool
that takes a pasted socket listing and normalises it. The field splits into three adjacent groups,
and the table-stakes set is the union of what those three do.

| # | Group | Representative sources | What it is |
|---|---|---|---|
| 1 | The capture commands + the how-to articles that teach them | linuxize, tecmint, baeldung, nixCraft, cyberciti, oneuptime, helpdeskgeek "netstat -ano" walkthroughs | `ss -tulpn` / `netstat -tulpn` / `netstat -ano` / `lsof -i -P -n` and grep. Produce the input; no normalisation, no cross-platform table, no conflict detection |
| 2 | Desktop socket-viewer GUIs (Windows) | TCPView, CurrPorts (documented feature lists) | Live local socket table: process name/PID/protocol/local port columns, click-header sorting, include/exclude filters on port/protocol/address/process, right-click "close connection"/"kill process", export to HTML/XML/tab-delimited, colour-marking of unidentified owners |
| 3 | Port-number lookup sites | port-lookup.utils.com, whatismyip port lookup, top10k, wintelguy, adminsub, speedguide | Number → service name from the IANA registry plus common unofficial ports. One port at a time, no process context |
| — | CLI wrappers | `cdzombak/listening` | A consistent cross-platform front-end over `lsof`/`netstat` — the closest thing to our normalisation idea, but it runs the commands itself and is not usable from a captured paste |

Group 1 tells us which dialects must parse. Group 2 defines the table/columns/filters/export UX
users already expect. Group 3 is a feature we can fold in for free instead of making the user
open a second tab per port.

## Table-stakes inventory

Every item below ends up either in our descriptor/page or in an explicit "not built" list —
nothing is dropped silently.

### In-model — shipped

| Capability | Seen at | Our param / behaviour | Our default |
|---|---|---|---|
| Parse `lsof -i` output | 1 | `input_format` = `lsof` | covered by `auto` |
| Parse `ss -tulpn` output | 1 | `input_format` = `ss` | covered by `auto` |
| Parse Linux `netstat -tulpn` output | 1 | `input_format` = `netstat` | covered by `auto` |
| Parse Windows `netstat -ano` / `-anb` (incl. the image name on the following line) | 1, 2 | `input_format` = `netstat-windows` | covered by `auto` |
| Not making the user say which command they ran | — (our differentiator) | `auto` scores the paste against all four dialects | `auto` |
| Process name + PID column | 1, 2 | always present, normalised across dialects | — |
| Protocol / local address / local port / state columns | 1, 2 | always present; `tcp6`/`udp6` derived from the address family | — |
| User/owner column | 2 (CurrPorts "user that created it") | present when the dialect carries it (`lsof`), `-` otherwise | — |
| Column sorting | 2 (click-header sort) | `sort_by` = `port` \| `pid` \| `process` \| `state` \| `address`; port sorts **numerically** (8 < 9 < 80) | `port` |
| Include/exclude filters on port | 2 (advanced filters) | `ports` — comma list + inclusive ranges (`80,443,8000-8100`) | `""` (all) |
| Filters on protocol | 2 | `protocol` = `any` \| `tcp` \| `udp` | `any` |
| Filters on process | 2 | `process` — case-insensitive substring | `""` (all) |
| "Show listening only" | 1 (`-l`/`LISTENING` grep is in every article), 2 | `listening_only` | `true` |
| Port → well-known service name | 3 | `annotate_services` adds a Service column, in-table, for every row at once | `true` |
| Kill the process holding a port | 2 (right-click kill) | `kill_commands` prints ready-to-run `kill -9 <pids>` / `taskkill /PID … /F` per port | `false` |
| Export the table | 2 (HTML/XML/tab-delimited) | `output_format` = `markdown` \| `text` \| `csv` \| `json` | `markdown` |
| Highlighting rows that need attention | 2 (pink for unidentified owners) | Conflict column + a per-port conflict list | — |
| Preset / "load a sample" | common on paste-in tools generally | four `[[example]]` chips on the page | — |

**Deliberate differentiator — conflict detection.** No source in any of the three groups answers
"which port is bound twice?". Groups 1 and 2 show a live list and leave the grouping to the reader;
group 3 knows nothing about processes. We group by protocol + port and flag ports held by two
*distinct* programs, which is the actual question behind `EADDRINUSE`. The two false-positive
shapes are excluded on purpose: a worker pool (same command name, many PIDs, one socket) and a
dual-stack bind (same PID on `0.0.0.0` and `::`).

**Second differentiator — works from a paste.** Group 2's tooling is Windows-only, installed, and
local-machine-only. A pasted capture comes from anywhere: a container, a CI runner, a server over
SSH, a colleague in a ticket. That is also why the page can stay browser-local: the privileged
capture already happened elsewhere, and nothing here is executed.

### In-model but intentionally not built (listed, not dropped)

| Capability | Seen at | Why not |
|---|---|---|
| Remote/foreign address as a first-class filter | 2 | The peer column is already shown for connected sockets when present, but a listening-port mapper filtering on peers is a different tool (connection auditing). Kept out of an 11-param form. |
| Full executable path, version info, process creation time | 2 | Not present in any of the four capture dialects, so it cannot be recovered from the paste — inventing it would be worse than omitting it. |
| Colour-coding suspicious/unidentified owners | 2 | The equivalent signal (`-` for an unknown PID/process, and the Conflict column) is already in the table; per-cell colouring belongs to the shared page renderer, not one block. |
| HTML / XML export | 2 | Markdown, aligned text, CSV and JSON cover the paste-into-a-ticket, terminal, spreadsheet and script cases. XML has no consumer here. |
| Group-by-process rollup ("this PID owns 6 ports") | 2 (implicit in its process column sort) | `sort_by = process` already clusters them, and a second output shape would double the render surface for a presentational gain. |
| IANA registry completeness (all ~1,500 registered names) | 3 | The table covers the registrations an operator actually meets plus the unregistered dev-server ports (3000, 4200, 5173, 8080, 9229 …), which are the ones a lookup site *misses*. A full registry dump would add weight for names nobody sees on a host. |
| Live "re-scan" / auto-refresh | 2 | There is no socket to poll — the input is a paste. |

### Out-of-model (cannot be done by this block at all)

| Capability | Seen at | Why |
|---|---|---|
| Reading the local machine's own socket table | 1, 2 | A browser tab cannot enumerate sockets; it needs a privileged local process. The user runs the capture command and pastes. |
| Actually killing a process / closing a connection | 2 | Same reason. We emit the exact command line instead, which is also the safer default — the user reviews the PID before running it. |
| Continuous monitoring, alerting, history | 2 | Needs a resident agent and storage; this is a one-shot, no-account, no-server tool. |
| Resolving a PID to a running process after the fact | 2 | The PID is only meaningful on the host that produced the capture. |

## Decisions recorded

- Defaults chosen to make the first render useful with zero clicks: `auto` detection,
  `listening_only = true` (the noisy ESTABLISHED rows are what people grep away by hand),
  `annotate_services = true` (free context, one column), `kill_commands = false` (destructive
  commands should be asked for, not offered unprompted).
- `conflicts_only` is off by default: the conflict list is already surfaced under the full table,
  so the narrow view is an opt-in drill-down rather than a mode you land in.
- Service annotation is a *column*, not a separate lookup mode — folding group 3's whole product
  into one column of group 2's table is the cheapest real win in this scan.
