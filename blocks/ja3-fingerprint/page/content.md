## About this tool

**JA3 Fingerprint Calculator** computes the **JA3** fingerprint of a TLS client
from its **ClientHello**, given as a **hex string**. JA3 is a widely used way to
identify the software behind a TLS connection without decrypting it — the same
client library tends to produce the same JA3 regardless of destination, which is
why it shows up in IDS / proxy / threat-intel pipelines.

### How JA3 is built

JA3 concatenates **five decimal fields** taken from the ClientHello, in this
exact order, separated by commas:

```
SSLVersion,Ciphers,Extensions,EllipticCurves,EllipticCurvePointFormats
```

- **SSLVersion** — the ClientHello `legacy_version` (e.g. `771` = `0x0303` = TLS 1.2).
- **Ciphers** — the offered cipher suites as decimal numbers, joined by `-`.
- **Extensions** — the extension types present, decimal, joined by `-`.
- **EllipticCurves** — the `supported_groups` (extension 10) named groups.
- **EllipticCurvePointFormats** — the `ec_point_formats` (extension 11) values.

The fields are joined into the **JA3 string**, and the **MD5** of that string is
the **JA3 hash** you'll see in logs (32 hex characters).

### GREASE is removed

Per **RFC 8701**, modern clients sprinkle reserved **GREASE** values (`0x0a0a`,
`0x1a1a`, … `0xfafa`) into their cipher suites, extensions and groups to keep the
ecosystem tolerant of unknown values. JA3 ignores these so the fingerprint stays
stable; this tool removes them automatically before building each field.

### JA3N — the normalized variant

Modern browsers (Chrome 110+, Firefox 114+) **randomize the order of their TLS
extensions** on every connection, so the same browser can produce different JA3
hashes from one request to the next. **JA3N** fixes this: it is identical to JA3
except the **extension list is sorted ascending** before hashing, so the
fingerprint stays stable regardless of extension permutation. This tool returns
both JA3 and JA3N (string + MD5) — use JA3N when you need a fingerprint that
survives extension randomization.

### What you get

Alongside the JA3 and JA3N strings and their MD5 hashes, the result includes the
legacy TLS version, the decimal lists of cipher suites, extensions, elliptic
curves and point formats, and any **SNI** server names carried in the
ClientHello.

### Where to get the bytes

A ClientHello is the first message a client sends. In Wireshark, expand the
**TLSv1.x Record Layer → Handshake Protocol: Client Hello** and copy the bytes
(right-click → *Copy → …as a Hex Stream*), or take them from a `tcpdump` /
`openssl s_client` capture. You can paste the whole TLS record (starting
`16 03 ...`), just the handshake message (starting `01 ...`), or the ClientHello
body directly — all three are accepted. Spaces, colons, dashes, dots, commas,
and a leading `0x` are ignored.

### Common uses

- Compute the JA3 of a captured ClientHello to match it against a threat-intel
  feed or block-list.
- Confirm that a client library or scanner produces the JA3 you expect.
- Understand *why* two clients share (or differ in) a JA3 by inspecting the
  underlying cipher suites, extensions and curves.

### Notes

JA3 is a heuristic, not a cryptographic identifier — different clients can
collide on the same JA3, and a client can deliberately randomize its
ClientHello. Treat a JA3 match as a signal, not proof.
