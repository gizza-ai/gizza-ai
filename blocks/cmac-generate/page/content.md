## About this tool

This AES-CMAC generator computes a **block-cipher message authentication code**
from a message and a secret AES key, right in your browser. The computation runs
locally in WebAssembly — your message and key are **never uploaded** to a server,
which makes it safe for symmetric keys and other sensitive values.

CMAC (**Cipher-based MAC**, standardised in **NIST SP 800-38B** and **RFC 4493**)
keys the AES block cipher, derives two subkeys, and runs a CBC-MAC-style chain
over the padded message to produce a 16-byte tag. Like any MAC, the tag **cannot
be recomputed or forged without the key**, so it proves both that a message is
unaltered (integrity) and that it came from someone holding the key
(authenticity). CMAC is used where a block cipher — rather than a hash — is the
available primitive: IoT and smart-card protocols, IEEE 802.1X / RADIUS, and
key-derivation schemes.

### Which AES variant?

CMAC's underlying cipher is chosen by the **key length**:

- **16 bytes** → **AES-128-CMAC** (32 hex chars)
- **24 bytes** → **AES-192-CMAC** (48 hex chars)
- **32 bytes** → **AES-256-CMAC** (64 hex chars)

A plain text key is used as its raw UTF-8 bytes, so it must be exactly 16, 24, or
32 characters long; for any other binary key, set **Interpret key as** to **hex**
or **base64**. The output tag is always 16 bytes (128 bits) regardless of variant.

### Options

- **Interpret message as** / **Interpret key as** — read each input as plain
  UTF-8 text (default), or decode it from **hex** or **base64** first. Use
  hex/base64 for binary keys or pre-encoded payloads.
- **Output format** — return the tag as **hex** (default) or **base64**.
- **Uppercase hex** — emit the hex tag in uppercase.

### Notes

- CMAC is a one-way function: the tag cannot be reversed back into the message or
  the key.
- To verify a tag, recompute the CMAC over the same message and key and compare
  it (ideally with a constant-time comparison) against the expected tag.
- The empty message is valid — AES-128-CMAC of the empty message with the RFC
  4493 example key is `bb1d6929e95937287fa37d129b756746`.
- For a **hash-based** keyed MAC (HMAC-SHA256, etc.), use the HMAC generator
  instead. For an **unkeyed** hash, use the text hash generator.
