## What this tool does

The **Disposable Email Detector** tells you whether an email address (or a bare
domain) belongs to a known *disposable*, *temporary*, or *throwaway-inbox*
provider — the kind of self-destructing mailbox people use to sign up without
giving out their real address. It runs entirely in your browser: nothing is sent
to a server, it works offline, and there is no sign-up. It is also
**lookup-only** — it never performs a DNS, MX, or SMTP query, so it answers
"is this a known burner domain?", not "does this mailbox exist?".

Paste an address (a `Name <addr>` or `mailto:addr` wrapper is unwrapped
automatically) or just a domain, and it reports the domain it parsed, whether
that domain is disposable, the specific provider it matched, and a short reason.

## How it decides

| Signal | Example caught |
| --- | --- |
| **Exact domain match** | `user@mailinator.com`, `me@guerrillamail.com` |
| **Alias domains** | `sharklasers.com`, `grr.la`, `spam4.me` (Guerrilla Mail) |
| **Subdomains** | `inbox.mailinator.com` → matches `mailinator.com` |
| **Throwaway keyword in a label** | `tempmail.example.io`, a `throwaway` label |
| **Bare domain input** | `yopmail.com` (no local part needed) |

The built-in list covers the most common throwaway services — Mailinator,
Guerrilla Mail, 10 Minute Mail, Temp-Mail, YOPmail, Maildrop, Trashmail, Getnada
and many of their alias domains. The keyword heuristic is deliberately
conservative: it only fires when a whole domain label *is* a throwaway keyword
(so `contemporary-art.org` is **not** flagged just because it contains "temp").

## Signal, not a guarantee

A **Disposable: no** result means the domain is *not on the known list* — it does
not prove the mailbox is real or that the domain is reputable. New throwaway
services launch constantly, and a determined user can always register a fresh
domain. Treat the result as a fast, private first-pass filter: it catches the
overwhelming majority of casual burner-inbox sign-ups before you ever touch the
network, and you can layer slower checks (DNS/MX, double opt-in) on top when a
hard guarantee matters.

## Why detect disposable addresses?

Disposable inboxes are great for users who want privacy, but they are a problem
when you need a durable way to reach someone: trial-abuse and one-account-per-user
limits get bypassed, transactional and password-reset emails bounce into a
mailbox that vanished minutes later, and your deliverability and list quality
suffer. Catching a throwaway domain at the point of sign-up — instantly,
privately, and right in the browser — lets you nudge the user for a permanent
address before the bad data ever lands in your database.

## FAQ

<details>
<summary>Does it verify that the mailbox actually exists?</summary>

No. The detector is lookup-only: it never performs a DNS, MX, or SMTP query.
It answers "is this domain a known burner provider?", so a **Disposable: no**
result means the domain is not on the built-in list — not that the mailbox is
real or deliverable. Layer a DNS/MX check or double opt-in on top when you
need a hard guarantee.

</details>

<details>
<summary>Can I check a bare domain, or does it need a full address?</summary>

Both work. You can paste `yopmail.com` on its own, a full address like
`user@mailinator.com`, or wrapped forms — `Name <addr@domain.tld>` and
`mailto:addr@domain.tld` are unwrapped automatically before the domain is
extracted and checked.

</details>

<details>
<summary>Are subdomains and alias domains detected too?</summary>

Yes. `inbox.mailinator.com` matches the parent `mailinator.com` entry, and the
list includes popular alias domains such as `sharklasers.com`, `grr.la`, and
`spam4.me` (all Guerrilla Mail). There is also a conservative keyword
heuristic that only fires when an entire domain label *is* a throwaway keyword
— so `tempmail.example.io` is flagged, but `contemporary-art.org` is not.

</details>

<details>
<summary>Is the address I paste sent to a server?</summary>

Never. The check runs entirely in your browser against a built-in domain list,
works offline, and no address, domain, or result leaves your machine.

</details>
