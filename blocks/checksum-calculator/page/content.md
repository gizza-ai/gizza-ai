## About this tool

CRC checksums are small integrity values used in file formats, storage systems,
network protocols, and embedded devices. Paste text, hex bytes, or base64 bytes,
choose the CRC variant, and this tool returns the checksum in padded hexadecimal
or decimal form. If you already have a checksum from a manifest, device log, or
protocol trace, paste it into **Expected checksum** to get a clear `MATCH` or
`MISMATCH` verdict.

The four built-in variants cover the common cases: **CRC-32/ISO-HDLC** for ZIP,
gzip, PNG, Ethernet-style checks; **CRC-32C/Castagnoli** for systems such as
iSCSI and ext4; **CRC-16/ARC** for classic 16-bit CRC workflows; and
**CRC-8/SMBUS** for simple 8-bit checks. The standard check string
`123456789` produces `cbf43926`, `e3069283`, `bb3d`, and `f4` respectively.

This is an error-detection checksum calculator, not a cryptographic hash tool.
Use it when a protocol or file format specifically calls for a CRC. For SHA,
BLAKE, MD5, or other digests, use the hash tools instead.

## Worked example

Input:

```text
123456789
```

Settings: `algorithm = crc32`, `input_encoding = text`, `output_format = hex`,
`expected = cbf43926`.

Output:

```text
CRC-32: cbf43926
Expected: cbf43926
Result: MATCH
```

## Limits and edge cases

- Text input is checksummed as UTF-8 bytes. For exact binary data, encode the
  bytes as hex or standard base64 first.
- Hex input may include whitespace and a leading `0x`; base64 input may include
  whitespace.
- Expected values can be hex with or without `0x`, any case, or a decimal number.
- CRCs detect accidental corruption; they are not safe for passwords, signatures,
  or tamper-resistant security checks.

## FAQ

<details>
<summary>Which CRC algorithm should I choose?</summary>

Use the variant required by the file format or protocol you are checking. ZIP,
gzip, PNG, and many Ethernet-style examples usually mean CRC-32/ISO-HDLC.
Storage and networking systems that mention Castagnoli usually mean CRC-32C.
Older serial or embedded examples often specify a named CRC-16 or CRC-8 variant;
this tool provides CRC-16/ARC and CRC-8/SMBUS.

</details>

<details>
<summary>Can I verify a checksum I already have?</summary>

Yes. Paste the known checksum into **Expected checksum**. The tool accepts hex
with or without a `0x` prefix, uppercase or lowercase, leading zeros, or a plain
decimal integer, then appends `Result: MATCH` or `Result: MISMATCH`.

</details>

<details>
<summary>How do I checksum raw bytes instead of text?</summary>

Set **Interpret input as** to `Hex bytes` or `Base64 bytes`, then paste the
encoded bytes. For example, the text bytes for `123456789` are hex
`313233343536373839` and base64 `MTIzNDU2Nzg5`; both decode to the same bytes as
the text input.

</details>

<details>
<summary>Is a CRC the same as a cryptographic hash?</summary>

No. CRCs are designed to catch accidental transmission or storage errors. They
are fast and useful for compatibility with file formats and protocols, but they
are not collision-resistant against an attacker. For cryptographic checksums,
use SHA-256 or another digest-oriented hash tool.

</details>
