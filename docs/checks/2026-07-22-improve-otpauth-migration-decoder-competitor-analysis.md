# otpauth-migration-decoder — competitor analysis (2026-07-22)

Function: decode a Google Authenticator `otpauth-migration://offline?data=…` export
payload (the batch-transfer QR content) into individual standard `otpauth://totp/…` /
`otpauth://hotp/…` provisioning URIs, one per account, preserving issuer / account /
algorithm / digits / counter / type and the base32 secret. Pure, offline, no network.

## Competitors scanned (top 3 real tools/implementations)

1. **Browser decoder (privacy-first React app; junedkhatri31 / merakisys demo).** Scans the
   export QR in-browser, base64-decodes the `data=` param, parses the protobuf, and lists each
   account with its secret and a regenerated `otpauth://` URL. Everything runs client-side;
   nothing is uploaded. Table-stakes learned: (a) accept the payload however the user has it —
   full `otpauth-migration://` URL or just the `data` value; (b) show the reconstructed
   `otpauth://` URI per account, not only the raw secret; (c) surface issuer + account name;
   (d) emphasise local-only processing.

2. **`otpauth` CLI (dim13, Go).** Converts an `otpauth-migration://offline?data=…` transfer
   link into plain `otpauth://` links (and can render each as a QR). Confirms the canonical
   output is a newline-separated list of standard single-account provisioning URIs with
   `secret`, `issuer`, `algorithm`, `digits`, and `period`/`counter` query params — exactly the
   Key Uri Format. Table-stakes: emit one line per account; map the protobuf algorithm/digits
   enums back to the URI's textual values (SHA1/SHA256/SHA512, digits 6/8); carry the HOTP
   counter only for HOTP entries.

3. **`otpauth_migrate` / `otp_export` (brookst, qistoph — Python, protobuf .proto).** Reference
   implementations that pin the wire format: `MigrationPayload { repeated OtpParameters
   otp_parameters = 1; int32 version = 2; batch_size/index/id = 3/4/5 }`, `OtpParameters {
   bytes secret = 1; string name = 2; string issuer = 3; Algorithm algorithm = 4; DigitCount
   digits = 5; OtpType type = 6; int64 counter = 7 }`. Enums: Algorithm 0=UNSPECIFIED, 1=SHA1,
   2=SHA256, 3=SHA512, 4=MD5; DigitCount 0=UNSPECIFIED, 1=SIX, 2=EIGHT; OtpType 0=UNSPECIFIED,
   1=HOTP, 2=TOTP. They also confirm the `secret` field holds RAW bytes that must be
   **base32-encoded** (RFC 4648) to appear in the URI, and that these tools also expose a
   structured/JSON view of the decoded accounts for import into password managers.

## Table-stakes → decisions (every one lands in the descriptor or is listed here)

| Capability | In-model? | Decision |
|---|---|---|
| Accept full `otpauth-migration://…` URL **or** bare `data=` base64 | yes | `payload` param accepts both; URL is parsed for its `data` query value, else the whole string is treated as the base64 payload |
| One standard `otpauth://` URI per account | yes | default `format = uri` → newline-separated list |
| Structured view (type/issuer/name/secret/algorithm/digits/counter) for import | yes | `format = json` → pretty JSON array incl. the reconstructed `uri` |
| Map algorithm enum → SHA1/SHA256/SHA512/MD5 | yes | done; unspecified → SHA1 |
| Map digit-count enum → 6/8 | yes | done; unspecified → 6 |
| Preserve HOTP counter, TOTP period | yes | counter emitted for HOTP; period=30 emitted for TOTP (payload carries no period; 30 is the app default) |
| Base32-encode the raw secret bytes (RFC 4648) | yes | `base32` crate, no padding — matches authenticator apps |
| Fully offline / secret never leaves the device | yes | pure block; no network; stated on the page |
| Robust errors on malformed base64 / truncated protobuf / wrong scheme | yes | explicit, actionable error messages + tests |
| Render each URI as a scannable QR image | **out-of-model** | this is a pure text tool; the sibling `otpauth-uri` + a QR tool cover QR rendering. Listed, not built. |
| Scan the export QR image directly (image input) | **out-of-model** | image decode belongs to a QR-decode tool; this tool takes the already-decoded payload text. Listed, not built. |

## Notes

- No copied competitor copy, branding, or trademarks — paraphrase only. "Google Authenticator"
  is named descriptively (the format's origin), as the competitors themselves do.
- Protobuf is decoded with a small hand-rolled wire reader (no approved wasm-safe protobuf crate
  in references; the two message types only need varint + length-delimited fields), so the block
  stays dependency-light and instantiates on every backend.
