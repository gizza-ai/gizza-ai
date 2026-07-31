# checksum-calculator — competitor analysis (2026-07-31)

Tool function: compute CRC-family checksums for text or encoded bytes, and optionally compare the result with an expected value.

Distinct from existing tools:
- `blocks/hash-all` and `blocks/hash-text` focus on cryptographic/message digests (SHA, BLAKE, MD5-family), not CRC-family error-detection variants.
- `blocks/verify-checksum` verifies cryptographic file/text digests against expected values; it does not expose CRC-8/CRC-16/CRC-32/CRC-32C variants.
- This tool is therefore viable in the current gizza model as a pure deterministic CRC calculator/verifier.

## Scan (top competitors, paraphrased — no copy/branding reproduced)

1. **emn178 Online Tools CRC calculator**. Offers text/file/URL-style input modes, a list of named CRC models, and custom CRC parameter controls. Table-stakes: named presets, text and byte-oriented input, clear checksum output, and enough model labeling to distinguish similar CRC names.
2. **CompuTools CRC calculator**. Emphasizes many CRC algorithms and input representations such as ASCII, hex, binary, and files. Table-stakes: CRC-8/CRC-16/CRC-32 family coverage, encoded-byte input, file-oriented use cases, and integrity-verification framing.
3. **crccalc.com**. Presents a compact CRC-8/CRC-16/CRC-32 online calculator with simple input and output for common variants. Table-stakes: quick defaults, known standard check values, and easy copyable results.
4. **Tool Slick CRC calculator**. Covers many CRC widths/implementations and explains checksums as transmission-integrity checks. Table-stakes: algorithm selection, width-specific output padding, and warning that CRC is not a cryptographic hash.

## Table-stakes → decision

| Table-stake | In/out model | Decision |
|---|---:|---|
| CRC-32 output for common ZIP/gzip/PNG/Ethernet workflows | in | `algorithm = crc32`, CRC-32/ISO-HDLC |
| CRC-32C / Castagnoli output | in | `algorithm = crc32c` |
| CRC-16 support | in | `algorithm = crc16`, documented as CRC-16/ARC |
| CRC-8 support | in | `algorithm = crc8`, documented as CRC-8/SMBUS |
| Text input | in | `text` + `input_encoding = text` default |
| Encoded byte input | in | `input_encoding = hex` and `base64` |
| Hex and decimal output | in | `output_format = hex` / `decimal` |
| Uppercase hex | in | `uppercase` checkbox |
| Compare against expected checksum | in | `expected` optional field reports MATCH/MISMATCH |
| Accept expected values with 0x/case/leading-zero differences | in | tolerant expected-value parser |
| Standard check vectors | in | docs + tests for `123456789` |
| Custom polynomial/init/reflection/xorout editor | out | too easy to misconfigure in a simple gizza tool; listed rather than built |
| Direct local file upload / URL fetch | out | page/CLI model is single text input; users can paste hex/base64 bytes |
| Hundreds of named CRC variants | out | current tool intentionally covers the four backlog variants; huge model matrix is not necessary for this slug |
| Cryptographic checksum verification | out | already covered by hash/verify-checksum tools; CRC page points users there |

## Descriptor / UX decisions

- Fixed-choice params use enums so the page renders selects for algorithm, input encoding, and output format.
- Preset chips cover the canonical CRC-32 check vector, CRC-32C uppercase verification, CRC-16 from hex bytes, and CRC-8 decimal from base64 bytes.
- The page copy explicitly names the variants and their standard check values so users can validate expectations without copying competitor language.
