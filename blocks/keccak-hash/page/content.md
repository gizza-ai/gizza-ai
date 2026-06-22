## About this tool

This Keccak hash generator computes the **original Keccak digest** of any text
right in your browser. The hashing runs locally in WebAssembly — your input is
**never uploaded** to a server, which makes it safe for keys, addresses, and
other sensitive strings.

A hash is a fixed-length fingerprint of its input. The same text always produces
the same digest, but the digest cannot be reversed back into the original — so
hashes are ideal for verifying that data has not changed, fingerprinting
content, and deriving identifiers.

### Keccak vs. SHA-3 — they are not the same

This tool implements the **original Keccak** algorithm (the one submitted to the
NIST hash competition), which uses `0x01` multi-rate padding. When Keccak was
standardized as **FIPS-202 SHA-3**, the padding was changed to `0x06`. As a
result, **Keccak-256 and SHA3-256 produce completely different digests for the
same input** — they are not interchangeable.

- Use **Keccak** (this tool) when you need the hash that **Ethereum** and the
  wider EVM ecosystem use.
- Use **SHA-3** (in the Text Hash Generator) when you need the NIST standard.

### Where Keccak-256 is used

Keccak-256 is the workhorse hash of Ethereum and EVM-compatible chains:

- Deriving **account and contract addresses**.
- The EVM **`KECCAK256`** opcode (often written `sha3` in older docs).
- Hashing transactions, blocks, and **storage / Merkle-Patricia trie** keys.
- Computing **4-byte function selectors** for the contract ABI (the first 4
  bytes of `keccak256("transfer(address,uint256)")`, etc.).

### Supported variants

- **Keccak-256** (default) — a 256-bit / 32-byte digest, the Ethereum hash.
- **Keccak-512** — a 512-bit / 64-byte digest.

### Options

- **Variant** — choose Keccak-256 or Keccak-512.
- **Interpret input as** — hash the text as plain UTF-8 (default), or decode it
  from **hex** (a leading `0x` is accepted) or **base64** first so you can hash
  existing raw bytes such as a key or calldata.
- **Output format** — return the digest as **hex** (default) or as **base64**.
- **Uppercase hex** — emit the hex digest in uppercase.

### Notes

- A hash is a one-way function: a digest cannot be reversed back into the
  original text.
- For the NIST SHA-3 family (SHA3-256, SHA3-512) and other algorithms (MD5,
  SHA-1, the SHA-2 family, BLAKE2, BLAKE3), use the Text Hash Generator tool
  instead.
