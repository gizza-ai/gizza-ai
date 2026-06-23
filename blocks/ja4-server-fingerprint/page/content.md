## About this tool

**JA4S Fingerprint Calculator** computes the **JA4S** fingerprint of a TLS server
from its **ServerHello**, given as a **hex string**. JA4S is the server half of
the **JA4+** suite (FoxIO): where JA3/JA4 fingerprint the client's ClientHello,
JA4S fingerprints the server's *response* — the version, cipher and extensions
the server chose. Pairing a client fingerprint with the server's JA4S gives a
fuller picture of a TLS session for threat-intel, scanning and inventory work.

### How JA4S is built

JA4S is an `a_b_c` string:

```
(t|q)(version)(extcount)(alpn) _ (cipher) _ (sha256_of_extensions)
```

- **transport** — `t` for TCP, `q` for QUIC.
- **version** — the negotiated TLS version as a 2-char code (`13`, `12`, `11`,
  `10`, `s3`, `s2`), taken from the **supported_versions** extension if the
  server sent one, otherwise from the ServerHello `legacy_version`.
- **extcount** — the number of extensions in the ServerHello, as 2 digits
  (capped at `99`).
- **alpn** — the first and last character of the **ALPN** protocol the server
  selected (e.g. `h2` → `h2`), `00` if none was negotiated, `99` if the value
  is non-ASCII.
- **cipher** — the single cipher suite the server chose, as 4 lowercase hex
  characters (e.g. `c02b`).
- **extension hash** — the first **12 hex characters** of the **SHA256** of the
  comma-joined extension-type list (each type as 4-char hex), in the order they
  appear on the wire. If the ServerHello has no extensions this is
  `000000000000`.

### GREASE is kept

Unlike JA3, JA4S **keeps GREASE values** in the extension list and does **not**
sort it — the extensions are hashed exactly in wire order. (GREASE, RFC 8701,
is the set of reserved `0x0a0a`, `0x1a1a`, … `0xfafa` values endpoints sprinkle
in to keep the ecosystem tolerant of unknowns.)

### TCP vs QUIC

The transport character is the only part you have to tell the tool, because it
isn't carried in the ServerHello bytes themselves. Leave the **QUIC handshake**
box unchecked for an ordinary TLS-over-TCP ServerHello (prefix `t`); check it
when the handshake was carried over QUIC (prefix `q`).

### What you get

Alongside the JA4S string, the result includes the **raw** variant `JA4S_r`
(the same `a` and `b` parts, but with the extension list shown un-hashed), the
negotiated TLS version, the chosen cipher, the full extension list, and the
selected ALPN protocol.

### Where to get the bytes

A ServerHello is the server's first handshake reply. In Wireshark, expand the
**TLSv1.x Record Layer → Handshake Protocol: Server Hello** and copy the bytes
(right-click → *Copy → …as a Hex Stream*), or take them from a `tcpdump` /
`openssl s_client` capture. You can paste the whole TLS record (starting
`16 03 ...`), just the handshake message (starting `02 ...`), or the
ServerHello body directly — all three are accepted. Spaces, colons, dashes,
dots, commas, and a leading `0x` are ignored.

### Common uses

- Compute the JA4S of a captured ServerHello to match it against a threat-intel
  feed or to fingerprint a server stack.
- Confirm that a server (or a TLS-terminating proxy / load balancer) produces
  the JA4S you expect.
- Pair a client JA4 with the server's JA4S to characterise a whole session.

### Notes

JA4S is a heuristic, not a cryptographic identifier — different servers can
produce the same JA4S, and a server's response can vary with the client's
offer. Treat a JA4S match as a signal, not proof. Everything runs locally in
your browser; the ServerHello bytes are never uploaded.
