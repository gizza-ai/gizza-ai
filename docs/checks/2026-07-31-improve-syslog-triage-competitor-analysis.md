# syslog-triage — competitor analysis (2026-07-31)

Scan done BEFORE implementation. Goal: parse/filter Linux syslog & auth.log and
highlight sudo, SSH auth, and cron events for a quick intrusion review. All notes
below are paraphrased from public docs/tools — no copy or branding reused.

## Competitors / references inspected

1. **DigitalOcean — "How To Monitor System Authentication Logs on Ubuntu"**
   (tutorial). Command-line workflow around `/var/log/auth.log`.
2. **Elastic Blog — "Grokking the Linux authorization logs"** (Grok-pattern
   reference for parsing auth logs into structured fields).
3. **GeeksforGeeks — "Find Failed SSH Login Attempts in Linux"** +
   ManageEngine EventLog Analyzer / nixCraft / SigNoz / Fail2ban / `sshaa`
   (secondary, for aggregation UX).

## Event categories the field treats as table-stakes

| Category | Trigger strings (paraphrased) | Fields commonly pulled |
| --- | --- | --- |
| **SSH auth** | `Accepted password/publickey for … from … port …`; `Failed password for [invalid user] … from … port …`; `Invalid user … from …`; `Connection closed … [preauth]` | user, source IP, port, method, result |
| **sudo** | `sudo: user : TTY=… ; PWD=… ; USER=target ; COMMAND=…`; `authentication failure`; `NOT in sudoers` | invoking user, target user, tty, command, result |
| **cron** | `CRON … (user) CMD (command)`; `pam_unix(cron:session): session opened for user …` | user, command |
| **session / su** | `pam_unix(<svc>:session): session opened/closed for user …`; `FAILED su for … by …` | user, service |
| **account mgmt** | `useradd… new user: name=…`; `groupadd… new group: name=…`; `passwd… password changed` | account name |

Universal syslog fields everyone extracts: **timestamp, host, service tag, PID,
username, source IP, result (success/failure)**. Two syslog line shapes matter:
BSD/RFC 3164 (`Mmm dd HH:MM:SS host tag[pid]: msg`) and rsyslog RFC 3339/ISO
timestamps; an optional `<PRI>` prefix can lead the line.

## Table-stakes params / defaults / UX controls

| Capability | Competitor norm | In this model? | Decision |
| --- | --- | --- | --- |
| Categorize sudo / ssh / cron (+ session, account) | grep per pattern | ✅ | Derive a `category` per event; `category` filter param (default `all`). |
| Show **failures only** (brute-force focus) | `grep "Failed password"` | ✅ | `only` filter (`all` / `failed` / `success`), derived per-event status. |
| Aggregate **failed logins by source IP** | `grep … \| awk '{print $11}' \| sort \| uniq -c` | ✅ | `summary` output ranks source IPs by failed-login count + the users tried. |
| Extract user / source IP / command / tty | Grok / awk field pos | ✅ | Parsed into `user`, `source_ip`, `detail` columns. |
| Machine-readable export | (EventLog/SigNoz dashboards) | ✅ | `output=json` (+ `table`) for pipelines/spreadsheets. |
| Cap rows for a big paste | pager / `head` | ✅ | `limit` param (default 500, max 5000). |
| Preset examples | docs give copy-paste greps | ✅ | Preset chips: SSH brute force, sudo activity, cron. |

## Out-of-model (intentionally NOT built)

- **Real-time monitoring / auto-ban** (Fail2ban) — this is a one-shot pure
  parser, not a daemon.
- **GeoIP / country lookup of source IPs** (`sshaa`, EventLog Analyzer) — needs a
  network/IP-database lookup; gizza blocks are offline pure-Rust.
- **`journalctl` querying / live tailing** — no host log access in a sandbox; the
  user pastes the log text.
- **Dashboards / charts / alerting** (SigNoz, ManageEngine) — out of scope for a
  text-in/text-out tool.

## Copy / UX notes applied

- Headline the three requested categories (sudo, SSH auth, cron) plus session &
  account for completeness.
- Default `summary` view leads with an intrusion-review header (`N events · M
  failed`) and a ranked "failed SSH logins by source IP" block — the single most
  common thing the field greps for.
- Placeholders on the paste box + numeric cap; enum selects for category / status
  / output; worked example + FAQs covering privacy, auth.log location, and the
  failure filter.
