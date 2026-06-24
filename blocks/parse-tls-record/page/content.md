## About this tool

**TLS Record Parser** decodes raw **TLS record-layer** bytes, given as a **hex
string**, into the record header and — for handshake records — the handshake
messages it carries.

Every TLS connection is framed as a series of records. Each record starts with a
**5-byte header**:

- **Content Type** — `change_cipher_spec` (20), `alert` (21), `handshake` (22),
  `application_data` (23), or `heartbeat` (24).
- **Record Version** — the record-layer protocol version (e.g. `TLS 1.0`,
  `TLS 1.2`). Note that under TLS 1.3 the record version is kept at `0x0303` for
  compatibility; the real negotiated version lives in the `supported_versions`
  extension.
- **Length** — the number of payload bytes that follow.

### Handshake decoding

When the content type is **handshake**, the payload holds one or more handshake
messages, each with a 1-byte type and a 3-byte length. **ClientHello** and
**ServerHello** are fully decoded:

- **Version** — the legacy_version field.
- **Random** — the 32-byte client/server random.
- **Session ID** — the (legacy) session id.
- **Cipher Suites** — named by their **IANA** identifier, e.g.
  `TLS_AES_128_GCM_SHA256`, `TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256` (a ClientHello
  lists the offered suites; a ServerHello reports the single selected one).
- **Compression Methods** — usually just `null`.
- **Extensions** — including **SNI** server names (`server_name`), **ALPN**
  protocols (e.g. `h2`, `http/1.1`), **supported_versions**, **supported_groups**
  (`x25519`, `secp256r1`, …), **signature_algorithms**, and **key_share** groups.

**Alert** records are decoded to their level (`warning`/`fatal`) and description
(e.g. `handshake_failure`, `bad_certificate`). **application_data** is encrypted,
so only the record header is shown.

TLS data on the wire is often **several records back-to-back** (for example a
ServerHello record followed by a Certificate record). Paste the whole stream and
each record is parsed and labelled in turn.

### Where to get the bytes

A TLS record is the bytes on the wire after the TCP header. In Wireshark, expand
the **TLSv1.x Record Layer** and copy the bytes (right-click → *Copy → …as a Hex
Stream*), or take them from a `tcpdump`/`openssl s_client` capture. Input may use
spaces, colons, dashes, dots, commas, or a leading `0x`; they are all ignored.

### Common uses

- Decode a captured **ClientHello** to see the offered cipher suites, SNI host,
  and ALPN protocols without re-opening the capture.
- Confirm the **selected** cipher suite and TLS version from a ServerHello.
- Read a TLS **alert** to find out why a handshake failed.
