## What this tool does

The **Email Normalizer** turns an email address into its *canonical* form — the
single deliverable address that a mailbox actually receives, with cosmetic and
provider-specific variations removed. It runs entirely in your browser: nothing
is sent to a server, it works offline, and there is no sign-up.

Paste an address (a `Name <addr>` or `mailto:addr` wrapper is unwrapped
automatically) and it reports the canonical address plus what was changed.

## What gets normalized

| Step | What happens |
| --- | --- |
| **Lowercase the domain** | DNS is case-insensitive, so `Example.COM` becomes `example.com`. |
| **Lowercase the local part** | On by default — virtually every provider treats the part before `@` case-insensitively. Turn it off to keep the original case. |
| **Strip the `+tag` sub-address** | `you+newsletter@…` and `you+shopping@…` both deliver to `you@…`, so the tag is removed (and reported). |
| **Remove Gmail dots** | Gmail ignores `.` in the local part, so `john.doe@gmail.com` is the same mailbox as `johndoe@gmail.com`. Dots are removed for Gmail only. |
| **Fold `googlemail.com`** | Google's old domain `googlemail.com` is folded to `gmail.com`. |

## Provider rules

Dot-stripping applies to **Gmail / Googlemail only**. Sub-address (`+tag`)
removal applies to every recognized provider — **Gmail, Outlook / Hotmail /
Live, Yahoo, iCloud, Fastmail, and Proton Mail** — and to any other domain, since
plus addressing is the de-facto standard. Other providers keep their dots.

## Examples

| Input | Output (canonical) |
| --- | --- |
| `John.Doe+newsletter@googlemail.com` | `johndoe@gmail.com` |
| `First.Last+promo@Outlook.com` | `first.last@outlook.com` |
| `User+tag@Yahoo.com` | `user@yahoo.com` |
| `  Jane Roe <mailto:Jane.Roe@gmail.com>  ` | `janeroe@gmail.com` |
| `a.b.c+x@Example.CO.UK` | `a.b.c@example.co.uk` |

## Why normalize email addresses?

- **Deduplicate sign-ups** — `john.doe+promo@gmail.com` and `johndoe@gmail.com`
  are the same person; comparing canonical forms catches duplicate accounts and
  abuse.
- **Match records** — reconcile a contact list where the same mailbox was typed
  several different ways.
- **Validate input** — the tool flags syntactically invalid addresses (missing
  `@`, no domain dot, bad characters) so you can catch typos.

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Does removing dots affect non-Gmail addresses?** No. Only Gmail ignores dots
in the local part, so dots are preserved for every other provider.

**What about the `+tag` part — is that mail lost?** No. A message to
`you+anything@…` is delivered to `you@…`; the tag is just a filtering label, so
the canonical address still reaches the same inbox.

**Can I keep the local part's capitalization?** Yes — untick *Lowercase the
local part*. The domain is always lowercased because DNS is case-insensitive.
