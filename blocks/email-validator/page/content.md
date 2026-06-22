## What this tool does

The **Email Validator** checks whether an email address is *syntactically*
valid and tells you exactly what is wrong when it isn't. It runs entirely in
your browser: nothing is sent to a server, it works offline, and there is no
sign-up. It is also **syntax-only** — it never performs a DNS, MX, or SMTP
lookup, so it confirms the address is *well-formed*, not that the mailbox
actually exists.

Paste an address (a `Name <addr>` or `mailto:addr` wrapper is unwrapped
automatically) and it reports whether the address is valid, the local part and
domain it parsed, every hard error, every soft warning, and — when it spots a
likely typo — a suggested correction.

## What it checks

| Check | Example caught |
| --- | --- |
| **Missing or duplicated `@`** | `userexample.com`, `a@b@c.com` |
| **Empty or over-long parts** | `@example.com`, a local part over 64 characters |
| **Leading / trailing / consecutive dots** | `.user@x.com`, `user.@x.com`, `a..b@x.com` |
| **Illegal characters** | a space, `_` in the domain, non-ASCII without IDN |
| **Bad domain** | no dot (`user@localhost`), a label edge hyphen, an empty label |
| **Misspelled provider** | `user@gmial.com` → suggests `user@gmail.com` |
| **Misspelled TLD** | `user@example.con` → suggests `user@example.com` |
| **Suspicious TLD** | an all-numeric top-level domain |
| **Formatting noise** | surrounding whitespace, a quoted local part, an IP-address literal |

## Valid vs. warning

A result is marked **Valid: yes** when there are no hard syntax errors. It can
still carry *warnings* — for example `user@gmial.com` is perfectly well-formed,
but `gmial.com` is almost certainly a typo for `gmail.com`, so the tool flags it
and offers the corrected address as a suggestion. Warnings never make an address
invalid; they just point out things worth a second look before you hit send.

## Why validate format only?

Real-time DNS/MX or SMTP verification requires a network round-trip to the
recipient's mail server, which is slow, often rate-limited, and frequently
blocked by anti-abuse measures. Catching the obvious syntax mistakes and typos
*before* you send — instantly and privately, right in the browser — fixes the
large majority of bad addresses (fat-fingered domains, stray spaces, missing
`@`) without ever leaking the address to a third-party service.
