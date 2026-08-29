# email-validate — competitor analysis (2026-08-29)

Snapshot taken **before** implementation, per `/improve-tool` Phases 2–3 (run inline as part of the
`/create-next-tool` build of a brand-new tool, so the "gap list" is really a *build* list).

All competitor notes below are **paraphrased observations of behaviour and feature surface**. No
competitor copy, branding, wording, logos, or trademarks are reproduced or reused anywhere in this
tool.

## Scope of the scan

Two adjacent product categories serve the backlog row
(`Validate email address syntax and check the domain's MX records for deliverability plausibility`):

1. **MX / DNS lookup tools** — take a *domain*, show its mail-exchanger records.
2. **Email verifiers** — take an *address*, chain syntax → domain/MX → SMTP mailbox probe.

This tool sits deliberately between the two: address in, syntax + MX out, **no SMTP probe**.

## Top 3 competitors scanned

Reachability note: `dnschecker.org/mx-lookup.php`, `whatsmydns.net/dns-lookup/mx-records` and
`tools.emailhippo.com` all returned HTTP 403 to the fetcher, so they were replaced by the next real
tools in the search results rather than being counted as scanned. Three profiles below are from
pages that actually loaded.

### 1. MxToolbox — MX Lookup (`mxtoolbox.com`)

| aspect | observation (paraphrased) |
| --- | --- |
| input | a domain name |
| output | mail-exchanger records listed in priority order |
| chained checks | mail-server connection test, reverse DNS, open-relay scan, response-time measure, and a blacklist/DNSBL sweep across ~105 lists |
| resolver choice | not exposed on the lookup form |
| limits | none stated on the free lookup page |
| free vs paid | free lookup; monitoring/diagnostics sit behind a broader paid delivery product |
| positioning | "diagnose why mail is not being delivered" rather than "is this address good" |

### 2. UptimeRobot — Free MX Lookup (`uptimerobot.com/free-tools/mx-lookup/`)

| aspect | observation (paraphrased) |
| --- | --- |
| input | a domain name |
| output fields | mail-server hostname, priority value, resolved IP address, TTL |
| resolver choice | none — fixed to their own infrastructure |
| explanatory copy | how priority ordering works (lowest number tried first, higher numbers are fallbacks), and why several records give redundancy |
| limits | none stated; no signup |
| FAQ topics | interpreting the priority number, where to run the lookup, what multiple records mean |

### 3. Mailmeteor — Email Checker (`mailmeteor.com/email-checker`)

| aspect | observation (paraphrased) |
| --- | --- |
| input | one email address (bulk only via a spreadsheet add-on) |
| checks advertised | 15+, spanning syntax, disposable domains, role addresses, DNS/MX records, an SMTP handshake, and catch-all detection |
| verdict shape | a status label — valid / risky / invalid / unknown — plus a score and a short explanation |
| limits | free single checks under a fair-use policy; the spreadsheet add-on is capped monthly; bulk goes to paid partners |
| FAQ topics | how verification works, what each status means, why a "valid" address can still bounce, catch-all behaviour, privacy of submitted addresses |

## Table stakes extracted (→ what this tool must ship)

| table stake | seen at | in this tool |
| --- | --- | --- |
| accepts a full address, not just a domain | Mailmeteor | **yes** — `email` is the only required param; the domain is extracted |
| syntax check before any lookup | Mailmeteor | **yes** — reuses `blocks/email-validator`'s RFC 5321/5322 core; a syntactically dead address skips DNS entirely |
| MX hostname + preference, sorted | all three | **yes** — records sorted by preference ascending, then host |
| TTL shown | UptimeRobot | **yes** — per record, from the DoH answer |
| resolved IPs for each mail host | UptimeRobot, MxToolbox | **yes**, opt-in — `resolve_ips=true` runs an extra A query per host (off by default: it costs one round trip per host) |
| priority ordering explained, not just printed | UptimeRobot | **yes** — the report names the primary host and describes the fallback order |
| single verdict label, not raw records | Mailmeteor | **yes** — `verdict` (pass/fail) + `risk` (low/medium/high), matching the sibling `email-syntax-validator` vocabulary |
| "valid ≠ deliverable" caveat | Mailmeteor FAQ | **yes** — every report ends with an explicit note that no mailbox probe was made |
| typo suggestion on a misspelled provider domain | Mailmeteor (implicitly, via risky status) | **yes** — inherited free from `email-validator` (`gmial.com` → `gmail.com`) |
| machine-readable output | none of the three (all HTML) | **yes** — `format=json` (a gizza-family convention, and a genuine gap in all three) |

## Defaults chosen (and why)

| param | default | reasoning |
| --- | --- | --- |
| `resolver` | `google` | `dns.google/resolve` answers the JSON DoH form without a custom `Accept` header and was the more reliable of the two from the sandbox; `cloudflare` is offered as the alternate |
| `max_records` | `10` | large providers publish 5 MX records; 10 covers real domains without unbounded output. Range 1–50 |
| `fallback_a` | `true` | RFC 5321 §5.1: a domain with no MX but an A/AAAA record still accepts mail at that address (implicit MX). Off by default would report false "undeliverable" for small self-hosted domains |
| `resolve_ips` | `false` | one extra DNS round trip per host; useful for diagnostics, wasteful for the common "is this address plausible" question |
| `format` | `report` | matches every other gizza text tool; `summary` is the one-liner, `json` the machine form |

## Worked examples the tool ships

- `ada@gmail.com` → 5 MX hosts, primary `gmail-smtp-in.l.google.com` (preference 5), verdict pass / risk low.
- `user@gmial.com` → syntax valid, typo suggestion `user@gmail.com`, no MX for the typo domain → verdict fail.
- `bob@example.com` → RFC 7505 null MX (`0 .`) → the domain explicitly accepts no mail → verdict fail / risk high.

## In-model vs out-of-model

**In-model (built):** local syntax validation, domain extraction, DoH MX query over the wired HTTP
fetch, preference sorting, TTL, null-MX (RFC 7505) detection, implicit-A fallback (RFC 5321 §5.1),
optional per-host A resolution, record cap, resolver choice, report/summary/json output, verdict +
risk grading, typo suggestion pass-through.

**Out-of-model (considered, not built):**

- **SMTP mailbox verification / catch-all detection** (Mailmeteor, Email Hippo). Needs a raw TCP
  connection on port 25 from a reputable IP. The runtime offers HTTP only, and probing strangers'
  mail servers from a user's browser is not a thing this tool should do.
- **Raw recursive DNS from the block.** No UDP/TCP socket capability; DNS-over-HTTPS to a public
  resolver is the whole reason this tool is buildable at all.
- **Arbitrary/private resolver choice** (e.g. "query the authoritative nameserver", "use 8.8.4.4").
  Only resolvers that speak the JSON DoH form over HTTPS are reachable, so the choice is an enum of
  two known-good public endpoints, not a free-text nameserver field.
- **Blacklist / DNSBL sweep, open-relay scan, reverse DNS, response-time probing** (MxToolbox).
  DNSBL is arguably reachable over DoH, but it is a *domain reputation* tool, not address
  validation — a separate tool, not a param on this one.
- **Bulk / list verification.** The single-address shape is the family norm; list hygiene is already
  served by `blocks/email-list-cleaner`.
- **Disposable-domain and role-address flags.** Genuinely in-model, but already shipped by
  `blocks/email-syntax-validator` (which reuses `blocks/disposable-email-detector`). Duplicating
  them here would make two tools with the same answer; the page/CLI copy points at the sibling
  instead. This is a *considered, rejected* item, not a missing capability.
- **Historical/propagation view across many resolvers** (whatsmydns-style). Would need N parallel
  DoH queries and a caching story; low value for address plausibility.

## Positioning

Against MX lookup tools: this one takes an **address** and answers a plausibility question, instead
of dumping a record table for a domain. Against email verifiers: it is **honest about not probing
the mailbox** — no SMTP handshake, so a `pass` means "well-formed, and the domain has somewhere to
deliver to", never "this mailbox exists". That caveat is printed in the output itself, not buried in
an FAQ.
