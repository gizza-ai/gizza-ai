## About this tool

**PII Tokenizer** scans a block of text and replaces personally-identifiable
information with **deterministic, format-preserving pseudonyms**. Unlike masking or
redaction, the tokens keep the original shape *and* stay consistent: the same value
always maps to the same token, so you can share logs, datasets, or support tickets
that are de-identified yet still **linkable** — you can group, join, or de-duplicate
on the tokens without ever seeing the real data.

It detects and tokenizes:

- **Email addresses**
- **Phone numbers** (US-style, with optional country code / area-code parens)
- **IP addresses** — IPv4 (octets stay `0–255`) and IPv6 (groups stay valid hex)
- **Credit-card numbers** — 13–19 digit runs, **Luhn-validated** on the way in, and
  the token is re-Luhn'd so it stays a checksum-valid card number
- **US SSN-like numbers** (`123-45-6789`)

### Worked example

Input:

```
Contact Ada at ada@example.com or ada@example.com. Card 4111 1111 1111 1111.
```

Output (illustrative — the exact tokens depend on your secret):

```
Contact Ada at kqp7fh2x@example.com or kqp7fh2x@example.com. Card 3948 2071 6653 4890.
```

Notice the two copies of `ada@example.com` map to the **same** token, the domain is
preserved, the phone/card keep their punctuation and length, and the card is still
Luhn-valid. Run it again and you get the identical output.

### Options

- **Secret key** — scopes the deterministic mapping. The same secret always yields
  the same tokens, so datasets tokenized on different days still join. A *different*
  secret yields a completely different, unlinkable set — so keep your key private if
  you need the mapping to be stable and unguessable. Leave it blank to use a built-in
  default key.
- **Keep email domains** — on by default: only the local part (before `@`) is
  pseudonymized, so `@gmail.com` vs `@acme.com` stays visible for segmenting. Turn it
  off to pseudonymize the whole address.

### Privacy

Everything runs **in your browser** via WebAssembly — the text is never uploaded to a
server. You can also run it from the [gizza CLI](/) or inside a gizza chat, which
return per-category counts of what was tokenized.

### Limits & edge cases

- Detection is **pattern-based** (regex), so free-text PII like names, street
  addresses, and dates of birth is **not** caught — those need context-aware entity
  recognition this tool doesn't do. Review the output for high-stakes use.
- Tokens are keyed on the **exact matched text**, so the same phone written two ways
  (`(415) 555-0132` vs `415.555.0132`) produces two different tokens.
- Tokenization is **one-way**: it's a keyed HMAC, not a lookup vault, so there is no
  detokenize/re-identify step. It gives you linkability, not reversibility.

## FAQ

<details>
<summary>How is this different from the Redact PII tool?</summary>

[Redact PII](/tools/redact-pii/) *removes* data — it swaps each hit for a `[EMAIL]`
label or a run of `*`, which destroys linkability (every email looks the same after
redaction). This tool *pseudonymizes* instead: each distinct value becomes a
distinct, stable, same-shaped token, so you keep referential integrity — useful when
you still need to count unique users, join tables, or de-duplicate.

</details>

<details>
<summary>Is the tokenization reversible — can I get the original data back?</summary>

No. Tokens are produced with a keyed HMAC-SHA256 hash, which is one-way, so there is
no built-in detokenize step. That's deliberate: the goal is to share data that is
de-identified yet still linkable. True reversible tokenization needs a stored
token-to-value vault (or a reversible format-preserving cipher plus its key), which a
single-shot, in-browser tool doesn't keep.

</details>

<details>
<summary>What does the secret key do, and when should I set one?</summary>

The secret keys the deterministic mapping. Set the **same** secret every time and the
same value always maps to the same token, so datasets you tokenize on different days
stay joinable. Use a **different** secret and you get an entirely different, unlinkable
mapping. Set a private secret when you need the mapping to be stable across sessions
and hard for anyone else to reproduce; leave it blank for quick, self-consistent
scrubbing with the built-in key.

</details>

<details>
<summary>Are the tokens still valid — do cards pass validation and IPs stay in range?</summary>

Yes. Credit-card tokens are re-run through the Luhn checksum, so they remain
checksum-valid card numbers of the same length. IPv4 octets are mapped into `0–255`
(so the dotted quad is a valid-looking address), and IPv6 groups are substituted
nibble-by-nibble so each group stays valid hexadecimal. Phones, SSNs, and email
local parts keep their exact length and punctuation.

</details>

<details>
<summary>Does it detect names, street addresses, or dates of birth?</summary>

No. Matching is regex-based and covers emails, US-style phone numbers, IPv4/IPv6
addresses, Luhn-valid card numbers, and `123-45-6789`-style SSNs. Free-text PII like
names and postal addresses needs entity recognition, which this tool doesn't do —
proofread the output before sharing anything sensitive.

</details>
