## What this tool does

Paste a slice of a Linux **syslog** or **auth.log** file and get back a security
review instead of a wall of text. Every line is parsed into a structured event,
classified into a category — **sudo**, **ssh**, **cron**, **session**,
**account**, or **other** — and given a **success / failure** status. The user,
source IP, and command are pulled out so you can see who did what from where.

It understands both common line shapes: BSD syslog (RFC 3164, `Mmm dd HH:MM:SS
host tag[pid]: message`) and rsyslog ISO / RFC 3339 timestamps, with an optional
`<PRI>` priority prefix. Everything runs locally in your browser — the log text
is never uploaded.

## Worked example

Given this auth.log slice:

```
May  3 18:20:45 web1 sshd[2001]: Failed password for root from 203.0.113.5 port 44001 ssh2
May  3 18:20:47 web1 sshd[2002]: Failed password for invalid user admin from 203.0.113.5 port 44002 ssh2
May  3 18:21:10 web1 sshd[2010]: Accepted publickey for bob from 192.168.1.10 port 51000 ssh2
May  3 18:22:00 web1 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt-get update
May  3 18:25:01 web1 CRON[3001]: (root) CMD (/usr/local/bin/backup.sh)
May  3 18:26:00 web1 su[3100]: pam_unix(su:session): session opened for user root by alice(uid=1000)
```

the default **intrusion-review summary** leads with an events / failed header,
tallies each category, ranks failed logins by the IP they came from, and lists
sudo and cron activity:

```
Syslog triage · 6 events · 2 failed

Categories: sudo 1 · ssh 3 · cron 1 · session 1

Failed logins by source IP:
  203.0.113.5 ×2 (users: root, admin)

Sudo activity:
  alice ran (as root) /usr/bin/apt-get update

Cron:
  (root) ran /usr/local/bin/backup.sh
```

## Filters and output shapes

- **Category** keeps only one class of event — pick **ssh** to focus on login
  attempts, **sudo** for privilege escalation, **cron** for scheduled jobs, or
  leave it on **All categories**.
- **Status** narrows to **Failures only** (the fast path to brute-force attempts
  and denied `sudo`) or **Successes only**. Status is derived per event, not
  guessed from the whole file.
- **Output** switches between the **summary** above, a **Markdown table** with
  one row per event (`time`, `host`, `service`, `pid`, `category`, `status`,
  `user`, `source_ip`, `detail`), and a **JSON array** for piping into a script
  or spreadsheet.
- **Max events** caps how many events are rendered after filtering (default 500,
  hard maximum 5000) so a huge paste stays responsive.

## Limits and edge cases

- This is a one-shot parser, not a live monitor: it reads the text you paste, it
  does not tail `journalctl` or watch a file.
- Lines that don't match a syslog header aren't dropped — they become an
  `other` / `info` event carrying the whole line, so nothing is silently lost.
- Source-IP ranking counts **ssh** and **session** failures that carry an IP;
  a failure with no IP in the line simply isn't ranked.
- There is no GeoIP or country lookup — that needs a network database, and this
  tool is fully offline.

## FAQ

<details>
<summary>Is my log uploaded anywhere?</summary>

No. The parser runs entirely in your browser via WebAssembly. The log text you
paste stays on your machine and is never sent to a server.

</details>

<details>
<summary>Which log files does this work with?</summary>

Anything in standard syslog format. On Debian and Ubuntu, authentication events
live in `/var/log/auth.log`; on RHEL / CentOS / Fedora they're in
`/var/log/secure`. General system logs are in `/var/log/syslog` or
`/var/log/messages`. Paste any of them — the parser handles the BSD and ISO
timestamp styles either way.

</details>

<details>
<summary>How do I see only failed logins?</summary>

Set **Status** to **Failures only**. That keeps failed SSH passwords, invalid
users, and denied `sudo` / authentication failures, then the summary ranks the
source IPs behind them so a brute-force burst from one address is obvious at a
glance.

</details>

<details>
<summary>What's the difference between the summary and table output?</summary>

The **summary** is an at-a-glance security review — a header count, category
tallies, failed logins grouped by IP, and the sudo / cron sections. The
**table** (and **JSON**) are the raw per-event data: one row per parsed line
with every field, better for filtering further in a spreadsheet or feeding a
script.

</details>
