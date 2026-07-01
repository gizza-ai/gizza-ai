## What this tool does

**Defanging** rewrites an indicator of compromise (IOC) — a URL, IP address,
domain or email address — so that a mail client, chat app or terminal will not
turn it into a clickable link or run it. It's the standard safety practice when
you share malicious indicators in a threat report, a ticket, or an email to a
colleague. **Refanging** is the inverse: it restores a defanged blob back to the
real, clickable indicator so you can copy it straight into a sandbox or tool.

Everything runs locally in your browser — nothing is sent to a server, it works
offline, and there's no sign-up. Paste your text, pick an **Action**, and copy
the result.

## What gets neutralized

| Original | Defanged (square) |
| --- | --- |
| `http://evil.com` | `hxxp[://]evil[.]com` |
| `https://10.0.0.1` | `hxxps[://]10[.]0[.]0[.]1` |
| `evil.example.com` | `evil[.]example[.]com` |
| `bad.actor@evil.com` | `bad[.]actor[at]evil[.]com` |
| `ftp://files.bad.net` | `fxp[://]files[.]bad[.]net` |

It neutralizes three things: the **scheme**
(`http`/`https`/`ftp` → `hxxp`/`hxxps`/`fxp`), every **dot** between labels
(`.` → `[.]`), and the **`@`** in an email (`@` → `[at]`). The `://` separator is
bracketed too (`[://]`).

## Bracket styles

Pick the convention your team uses:

| Style | `.` | `@` | `://` |
| --- | --- | --- | --- |
| **square** (default) | `[.]` | `[at]` | `[://]` |
| **round** | `(.)` | `(at)` | `(://)` |
| **curly** | `{.}` | `{at}` | `{://}` |
| **dot** (spelled out) | `[dot]` | `[at]` | `[://]` |

## Refang

Switch the **Action** to **refang** to reverse the process. It recognizes square
`[]`, round `()` and curly `{}` brackets, the spelled-out `[dot]`/`[at]` forms,
and the `meow://` convention — so a blob copied from almost any report restores
cleanly:

| Defanged | Refanged |
| --- | --- |
| `hxxps[://]10[.]0[.]0[.]1` | `https://10.0.0.1` |
| `bad[at]evil[dot]com` | `bad@evil.com` |
| `meow://1.2.3.4` | `http://1.2.3.4` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and it
keeps working offline once the page has loaded. That matters when the text you're
handling contains live malicious indicators.

</details>

<details>
<summary>Why neutralize the scheme and dots?</summary>

A bare `http://evil.com` becomes a
clickable link in most apps, and an accidental click can detonate a payload or
leak that you visited an attacker's infrastructure. Replacing `http` with `hxxp`
and `.` with `[.]` breaks the auto-linking while keeping the indicator readable.

</details>

<details>
<summary>Does it work on a whole paragraph?</summary>

Yes. It rewrites the indicator characters
wherever they appear and leaves your surrounding prose untouched, so you can
paste a full sentence or a list of IOCs at once.

</details>

<details>
<summary>Will refang restore every defanged format?</summary>

It handles the common
conventions — `[.]`, `(.)`, `{.}`, `[dot]`, `[at]`, `[://]`, and `hxxp`/`fxp`
plus `meow://`. Exotic, hand-rolled obfuscations may need a manual touch-up.

</details>
