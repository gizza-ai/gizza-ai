## About this tool

The weak password detector checks a password against a **bundled, ranked list of
the most common and previously-breached passwords** — the ones that top public
breach compilations and that attackers try first in credential-stuffing and
brute-force attacks. Everything runs locally in your browser: the password is
never uploaded, logged, or sent to any server.

It goes beyond a plain string match. By default it also catches:

- **Case-only variations** — `PASSWORD`, `Password`, and `password` are all
  flagged, because attackers ignore case.
- **Leetspeak variants** — `P@ssw0rd`, `l3tm31n`, and similar digit/symbol
  substitutions collapse back to their base word (`0`→`o`, `@`→`a`, `1`→`i`,
  `3`→`e`, `4`→`a`, `5`→`s`, `7`→`t`, …), so dressing up a common password
  doesn't hide it.

Each match reports its **rank** (1 = most common), the list entry it matched,
how it matched, and a severity band (critical / high / common), so you can see
*why* the password is weak.

### Worked example

Enter `P@ssw0rd` with the defaults (case-insensitive, leetspeak on). The tool
collapses `@`→`a` and `0`→`o`, matches the bundled entry **“password”**, and
reports it as a leetspeak variant of one of the most common passwords ever
leaked — offering no real protection. Enter `123456` and it's flagged as the
**#1** most common password. Enter a long random passphrase like
`cor6rect$horse!Battery9Staple` and it comes back *not on the list*.

### Limits & edge cases

- This is a **bundled blocklist / dictionary check, not a live breach-database
  (Have I Been Pwned) lookup**. It does not call any API and does not know about
  every leaked password — only the well-known common ones shipped with the tool.
- A **“not found” result is not proof of strength.** It only rules out the
  best-known weak passwords. A short-but-uncommon password can still be weak by
  entropy; pair this with a strength/entropy check.
- An **empty input is rejected** — there's nothing to check.
- Matching is offline and deterministic: the same input always gives the same
  result.

## FAQ

<details>
<summary>Does this check my password against real breach databases like Have I Been Pwned?</summary>

No. This tool matches against a **fixed, bundled list** of the most common and
widely-published breached passwords, entirely offline — it never contacts any
API. That means it's private (nothing leaves your browser) but not exhaustive:
a password that isn't on the bundled list could still appear in a full breach
corpus. Treat a clean result as "not one of the obvious weak passwords", not as
"never breached".

</details>

<details>
<summary>Is my password sent anywhere?</summary>

No. The check runs locally in your browser via WebAssembly. Your password is
never uploaded, logged, or transmitted. You can confirm this by opening your
browser's network tab — there are no requests while you type.

</details>

<details>
<summary>What do "case-sensitive" and "leetspeak" do?</summary>

By default the tool is **case-insensitive**, so `PASSWORD` and `password` are
treated as the same weak password (attackers don't care about case). Turn on
**case-sensitive match** to require an exact-case hit. **Leetspeak detection**
(on by default) collapses common substitutions — `P@ssw0rd` becomes `password`
— so decorating a common password with symbols doesn't fool the check. Turn it
off to match only literal characters.

</details>

<details>
<summary>If my password isn't found, is it strong?</summary>

Not necessarily. "Not found" only means it isn't one of the well-known common
passwords in the bundled list. A short or predictable password (like a name plus
a year) can still be easy to guess even if it's not on any top-worst list. For
real strength, use a long, random, unique passphrase — ideally from a password
manager — and pair this blocklist check with an entropy/strength estimate.

</details>
