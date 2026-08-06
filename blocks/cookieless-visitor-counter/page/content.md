## About this tool

A cookie-free analytics counter can still answer the basic traffic question: how many unique people visited each day? This tool reads an access log and turns each request into a short SHA-256 visitor ID made from a salt, the chosen period, and identity material such as IP plus user-agent. Because the period is mixed into the hash, the same visitor gets a different ID the next day, so per-day IDs cannot be linked across days.

The raw IP, user-agent, and optional salt are never printed. The output is a report, table, JSON, CSV, or an audit view of pseudonymous IDs so you can check the method without storing cookies or PII.

### Worked example

Input log:

```
1.1.1.1 - - [06/Aug/2026:10:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"
1.1.1.1 - - [06/Aug/2026:10:05:00 +0000] "GET /a HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"
2.2.2.2 - - [06/Aug/2026:11:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Safari/604.1"
1.1.1.1 - - [07/Aug/2026:09:00:00 +0000] "GET / HTTP/1.1" 200 12 "-" "Mozilla/5.0 Chrome/125.0"
```

Result:

```
Cookieless visitor count
========================
Method:    daily-salted-hash (SHA-256), no cookies, no PII stored
Identity:  IP + user-agent
Bucket:    daily
Format:    Combined log

Date        Visitors  Pageviews  Views/visitor
----------  --------  ---------  -------------
2026-08-06         2          3           1.50
2026-08-07         1          1           1.00

Total pageviews:        4
Sum of daily uniques:  3
Distinct visitors:      2 (across the whole log)
Requests parsed:        4
Bot hits excluded:      0
```

The sum of daily uniques is 3 because one visitor returned on the second day and intentionally received a new daily ID. The whole-log distinct count is 2.

### Supported inputs and outputs

- **Formats:** auto-detected Apache/nginx Combined logs, Common logs, JSON/NDJSON lines, and CSV with a header row naming an IP column.
- **Identity modes:** IP + user-agent, IP only, or network + user-agent (IPv4 /24, IPv6 /48).
- **Periods:** hour, day, month, or whole log.
- **Outputs:** readable report, Markdown table, JSON, CSV, or pseudonymous IDs.
- **Bot handling:** crawler/script user-agents are excluded by default and can be counted by turning the checkbox off.

### Limits and edge cases

- A log can have up to 200,000 lines. Row-wise ID output is capped at 5,000 rows.
- Bot filtering is based on the declared user-agent only; it does not do reverse DNS or crawler IP-range verification.
- Shared NAT or office Wi-Fi can collapse many people into one visitor. A changing mobile IP or user-agent can split one person into several. This is inherent in log-based analytics.
- Re-run with `period=month` if you need monthly uniques; daily uniques do not add up to monthly uniques when the salt rotates by day.
- Use your own secret salt for real traffic. Leaving it blank makes results reproducible for demos and tests.

<details>
<summary>Does this store IP addresses or user-agents?</summary>

No. The tool is a pure WebAssembly function in your browser tab. It consumes IP and user-agent strings only to compute a SHA-256 digest, then drops the raw values. The report never prints them.

</details>

<details>
<summary>Why do daily visitor IDs change for the same person?</summary>

The period label is part of the hash input. That mimics a daily-rotated server salt: a person who appears on Monday and Tuesday cannot be linked by comparing IDs. The tradeoff is that daily unique counts should not be summed to get a monthly unique count.

</details>

<details>
<summary>Which identity mode should I choose?</summary>

Use IP + user-agent for the usual privacy-analytics convention. Use IP only if you need older AWStats-style counting. Use network + user-agent if you want to anonymize IPs before identification by truncating IPv4 to /24 and IPv6 to /48.

</details>

<details>
<summary>Can this replace a full analytics dashboard?</summary>

No. It counts visitors and pageviews from logs. It does not reconstruct sessions, bounce rate, geography, devices, referrers, campaigns, funnels, or persistent trends. Those need more data and storage than this stateless tool keeps.

</details>

<details>
<summary>How do I audit that no PII survives?</summary>

Choose the `ids` output. It lists only the period and a truncated hexadecimal visitor ID for each parsed request. The ID length is configurable from 6 to 64 hex characters.

</details>
