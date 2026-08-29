## About this tool

DKIM (DomainKeys Identified Mail, RFC 6376) lets a receiving server check that a
message really came from your domain and was not altered on the way. It works with
one key pair: your mail server signs outgoing mail with the **private** key, and the
**public** key is published in DNS as a TXT record so anyone can verify the signature.

This generator produces both halves at once:

- a **private key**, which you install on the signing server — as a PKCS#8 PEM and, for
  RSA, also as the PKCS#1 PEM that OpenDKIM and several ESPs expect;
- a **DNS TXT record** at `<selector>._domainkey.<domain>` containing the public key in
  the `p=` tag, plus the `v=`, `h=`, `k=` and optional `t=` tags around it.

Everything runs in WebAssembly inside this page, using your browser's cryptographically
secure random generator. No key is uploaded, stored, or logged. Nothing is published to
DNS for you either — the record is text you copy into your DNS provider.

## Worked example

Paste a key you already have into **Existing key** to rebuild the record for it. Using
this deliberately throwaway 1024-bit demo key (it protects nothing — never install it):

```
Domain:       example.com
Selector:     mail
Existing key: -----BEGIN PRIVATE KEY-----
              MIICdwIBADANBgkqhkiG9w0BAQEFAASCAmEwggJdAgEAAoGBAMlGx+vqjYM3RDkF
              … (the rest of the PEM)
              -----END PRIVATE KEY-----
```

With **Output** set to *TXT value only* the result is exactly the string your DNS panel
wants:

```
v=DKIM1; h=sha256; k=rsa; p=MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDJRsfr6o2DN0Q5BTItiaFdu6JuOT6CGHw8BUzNmHwYzxTQxtlZHvuE2nAkFi4vTrPSgUxtvEaWz+kQXBFd0wsZSkF8kPljems1jtM3U0yaYOavNQ7LYoQZJ9BZKN5sr3yrtsigaKECX895FBeEnAtFVCSgoI2N2AtcNpEMI2lecQIDAQAB
```

and *BIND zone-file line* gives the same value wrapped for a zone file:

```
mail._domainkey.example.com. 3600 IN TXT "v=DKIM1; h=sha256; k=rsa; p=MIGfMA0GCSqG…"
```

Leave **Existing key** blank instead and a brand-new key pair is generated, with the
private half shown above the record.

## Publishing the record

1. In your DNS provider, add a **TXT** record.
2. **Host / Name:** `mail._domainkey` (most panels append the domain for you; if yours
   wants the full name, use `mail._domainkey.example.com`). Replace `mail` with your
   selector.
3. **Value:** the whole `v=DKIM1; …` string, including the `p=` tag. Quotes are usually
   added by the panel — only a raw zone file needs them.
4. Install the private key on the signing server and tell it the same selector and
   domain, then send a test message and check the `Authentication-Results` header for
   `dkim=pass`.

DNS changes take up to the record's TTL to propagate, so wait before concluding that a
new selector is broken.

## Options

- **Domain** — the domain your mail comes from, which is also the `d=` value in the
  signature. A pasted URL, email address, or full record host is reduced to the domain.
  Internationalized domains must already be in punycode (`xn--…`) form.
- **Selector** — the label to the left of `._domainkey`, and the `s=` value in the
  signature. Any short ASCII label works (`mail`, `s1`, `2026a`). Use a *new* selector
  for every new key so a rotation never leaves mail unsigned.
- **Key type** — RSA 2048-bit is the interoperable choice. 1024-bit is legacy-only and
  now treated as weak; 4096-bit yields a TXT value well over 255 characters that some
  DNS panels and older resolvers refuse; Ed25519 (RFC 8463) produces a tiny record but
  is not verified by every receiver yet, so publish it *alongside* an RSA selector.
- **Output** — the full report, or just the piece you need: the TXT value, the zone-file
  line, the public key PEM, the private key, or a JSON object for scripting.
- **Add `h=sha256`** — publishes that only SHA-256 signatures are acceptable for this
  selector. On by default; turning it off shortens the record slightly.
- **Flag tag (`t=`)** — `t=y` marks the selector as being in test mode, so receivers must
  not treat a verification failure as a policy failure; `t=s` forbids using the key for
  subdomains. Production selectors normally publish neither.
- **Existing key** — paste a PKCS#8 or PKCS#1 private key, a base64 Ed25519 seed, a
  public key PEM, or a bare `p=` value to rebuild the record for a key that is already
  installed. The **Key type** choice is then ignored, because the pasted key decides it.

## Limits

- RSA key generation happens on this page's main thread; a 2048-bit key takes a moment
  and a 4096-bit key noticeably longer. Ed25519 is instant.
- Passphrase-encrypted (`BEGIN ENCRYPTED PRIVATE KEY`) and OpenSSH-format private keys
  are rejected — convert them with `openssl` first and paste the plain PEM.
- No DNS is queried or written: this tool cannot look up an existing selector, verify
  that a published record resolves, or create the record at your provider.
- It does not sign messages, parse `DKIM-Signature` headers, or generate SPF or DMARC
  records — those are separate jobs.
- Only `k=rsa` and `k=ed25519` are produced. There is no support for the optional `n=`,
  `g=` or `s=` service tags in the record.

## Security

The private key is the whole secret: anyone holding it can sign mail as your domain.
Generate it on a machine you trust, move it to the signing server over a secure channel,
store it readable only by the signing user, and never paste it into a ticket, a chat, or
a repository. Reloading this page discards the key, so copy it before navigating away —
there is no way to recover it afterwards, and a lost private key simply means generating
a new selector.

## FAQ

<details>
<summary>Which key size should I choose?</summary>

**RSA 2048-bit**, unless something forces otherwise. It is what the major mailbox
providers expect and what every verifier supports. 1024-bit keys still verify but are
widely regarded as too weak for a long-lived signing key. 4096-bit keys are stronger on
paper but make a TXT value that has to be split into multiple 255-character strings,
which some DNS panels and older middleboxes handle badly — the extra strength buys
little because DKIM keys are meant to be rotated, not kept for a decade.

</details>

<details>
<summary>Can I use the Ed25519 option today?</summary>

Yes, but not on its own. RFC 8463 added `k=ed25519` and it produces a very short record,
which sidesteps every 255-character DNS headache. Support among receivers is still
incomplete, so the standard advice is dual signing: publish an Ed25519 selector *and* an
RSA selector, and have your signer add both signatures. Receivers that understand
Ed25519 use it; everyone else falls back to the RSA signature.

</details>

<details>
<summary>My DNS provider rejects the value as too long. What now?</summary>

A single TXT string is capped at 255 characters (RFC 1035), and a 2048-bit RSA record is
around 400. Well-behaved panels accept the long value and split it internally; the
*BIND zone-file line* output shows the split form, with the value in several quoted
strings inside parentheses, which is what a raw zone file needs. If your panel accepts
neither, the practical fixes are to paste the value without spaces after the semicolons,
drop the `h=sha256` tag, or switch to an Ed25519 key, whose record fits in one string.

</details>

<details>
<summary>What is a selector, and can I reuse one?</summary>

The selector is just a label that lets one domain publish many DKIM keys — it is the
part before `._domainkey` in the record name, and your signer stamps it into each
signature as `s=`. Receivers combine `s=` and `d=` to find the right public key. You can
reuse a selector, but do not: when you rotate a key, publish the new key under a *new*
selector, switch the signer over, and only then remove the old record. Overwriting a
selector's record while mail signed with the old key is still in flight makes those
messages fail verification.

</details>

<details>
<summary>I already have a private key on my server. Can I get its record back?</summary>

Yes — that is what **Existing key** is for. Paste the private key and the record is
rebuilt from it, so the published `p=` value is guaranteed to match the key that is
actually signing. If you would rather not handle the private key at all, paste the
public key PEM instead; the record only ever contains the public half. Pasting a key
never generates a new one, and the **Key type** menu is ignored because the key itself
determines whether the record says `k=rsa` or `k=ed25519`.

</details>

<details>
<summary>Do I still need SPF and DMARC?</summary>

Yes. DKIM proves a message was signed by a key published under your domain, but on its
own it tells receivers nothing about what to do with unsigned or forged mail. DMARC is
the policy record that ties DKIM and SPF results to the visible `From:` domain and asks
receivers to quarantine or reject failures; SPF authorizes the servers allowed to send
for your domain. In practice all three are published together, and DKIM is the one that
survives forwarding, which is why DMARC alignment usually rests on it.

</details>
