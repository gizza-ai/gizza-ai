## Anonymize the IP addresses in a log without breaking the log

Paste a web-server access log, application log or any text and this tool rewrites **every valid
IPv4 and IPv6 address in place** — the surrounding request paths, status codes, user agents,
ports and timestamps are left exactly as they were. It runs entirely in your browser with
WebAssembly, so **the log never leaves your machine**.

Under GDPR (and the ePrivacy rules) a full IP address is **personal data**. Truncating or
pseudonymizing it before you store or share a log is the standard way to keep analytics and
debugging data while dropping the identifiable part. Pick the method that fits:

- **Mask** — zero the trailing octets/hextets, the way **Google Analytics** and **Matomo**
  anonymize IPs. `203.0.113.45` → `203.0.113.0` keeps country/city-level geolocation while
  dropping the individual host. Choose how many IPv4 octets (0–4) and IPv6 hextets (0–8) to
  zero: 1 octet is the GA default, 2 is Matomo's stricter setting, 5 hextets keeps the IPv6
  `/48`.
- **Hash** — replace each address with a short **salted SHA-256** token. The same address
  always maps to the same pseudonym (so you can still count unique visitors or correlate
  events) but the token is not reversible. Set a secret **salt** so nobody can rebuild the
  mapping with a rainbow table.
- **Redact** — swap in a fixed placeholder such as `[IP]` when you don't need to keep any
  structure at all.

Turn on **Skip private / internal addresses** to leave `10.x`, `192.168.x`, `127.0.0.1`,
`fc00::/7` and `fe80::/10` untouched — those aren't personal data, so masking them just makes
the log harder to read.

### Worked example

This Apache/Nginx access log line:

```
203.0.113.45 - - [10/Jul/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326
```

with **Mask** and 1 IPv4 octet becomes:

```
203.0.113.0 - - [10/Jul/2026:13:55:36 +0000] "GET /index.html HTTP/1.1" 200 2326
```

Only the address changed — the dash fields, bracketed timestamp, request line, status `200`
and byte count `2326` are byte-for-byte identical. Switch to **Redact** and the same line reads
`[IP] - - [10/Jul/2026:…`; switch to **Hash** with a salt and it reads
`<12-hex-token> - - [10/Jul/2026:…`, the same token every time that address appears.

### FAQ

<details>
<summary>Does masking an IP address make a log GDPR-compliant?</summary>

Truncating or hashing the IP is the recommended **data-minimization** step, and it is what the
German DPAs accepted for Google Analytics' `_anonymizeIp`. Full compliance still depends on your
lawful basis, retention and the rest of the record, but removing the identifiable part of the IP
is the single biggest step for a log. **Mask** (drop the last octet) keeps coarse geolocation;
**Hash** keeps per-visitor counting without storing the address itself.

</details>

<details>
<summary>Is my log uploaded anywhere?</summary>

No. The anonymizer is compiled to WebAssembly and runs entirely in your browser — the log text
is never sent to a server, logged or stored. You can even disconnect from the network and it
still works.

</details>

<details>
<summary>What's the difference between masking and hashing?</summary>

**Masking** zeros the trailing part of the address (`203.0.113.45` → `203.0.113.0`), so you keep
the network/region but two different hosts on the same subnet become indistinguishable.
**Hashing** replaces the whole address with a short salted token; different addresses stay
distinct (you can count unique visitors) but the value is not reversible. Use masking when you
want geolocation, hashing when you want per-visitor analytics without geo.

</details>

<details>
<summary>Why should I set a salt when hashing?</summary>

There are only ~4.3 billion IPv4 addresses, so an attacker can hash **all** of them and reverse
an unsalted token in seconds. A secret **salt** mixed in before hashing (`SHA256(salt : ip)`)
makes that table useless — they'd have to rebuild it for your specific salt, which they don't
have. Use one random salt per project and keep it private; reuse the same salt if you need the
pseudonyms to stay consistent across files.

</details>

<details>
<summary>Does it touch private or internal IPs?</summary>

By default every valid address is anonymized. Turn on **Skip private / internal addresses** and
RFC 1918 ranges (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), loopback (`127.0.0.1`,
`::1`), link-local and unique-local IPv6 (`fe80::/10`, `fc00::/7`) are left as-is — they aren't
personal data, so keeping them makes internal traffic easier to debug.

</details>

<details>
<summary>Does it preserve ports, IPv6 and IPv4-mapped addresses?</summary>

Yes. A trailing `:8080` port is kept (`203.0.113.45:8080` → `203.0.113.0:8080`), IPv6 addresses
are masked/hashed/redacted in their own right, and an IPv4-mapped IPv6 address like
`::ffff:192.0.2.1` is treated as **one** address and replaced once rather than double-processed.
Only text the standard-library parser accepts as a real IP is touched, so version numbers and
timestamps are left alone.

</details>

### Limits & edge cases

- Addresses are matched by pattern and then **validated** by the standard-library IP parser, so
  a token like `999.1.1.1` or a version string `1.2` is not mistaken for an address.
- **Mask** works on the parsed address, so masking re-emits the canonical form: masking the last
  5 hextets of `2001:db8:85a3::8a2e:370:7334` yields the compressed `2001:db8:85a3::`.
- **Hash** length is clamped to 4–64 hex characters; longer tokens mean fewer collisions. An
  empty salt still hashes but offers no protection against reverse lookup.
- IPv6 is resolved before IPv4, so an IPv4-mapped address is anonymized once as a whole and the
  inner dotted-quad is never double-replaced.
- The input must contain at least one character; an empty box reports a clear error rather than
  returning nothing.
