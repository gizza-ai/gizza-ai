# random-bytes — competitor analysis (2026-08-18)

Scan run before completing the tool, per `/create-next-tool` and `/improve-tool`. Everything below is paraphrased; no competitor copy, branding or trademarks are reused.

Search: "online random bytes generator hex base64 crypto secure random bytes OpenSSL rand alternative" (WebSearch, 2026-08-18). The results were a mix of browser random-key generators, OpenSSL-style generators, and command-line guides. Three reachable, real references were used to identify table-stakes behavior.

## Profiles

### 1. HexHero Random Key Generator — browser key/token generator

```json
{
  "name": "HexHero Random Key Generator",
  "url": "https://www.hexhero.com/tools/random-key-generator",
  "features": [
    "cryptographically secure key generation",
    "hex, Base64 and URL-safe token formats",
    "common presets for API keys, JWT/HMAC secrets, encryption keys and session tokens",
    "OpenSSL-style example framing for generated secrets"
  ],
  "params_options": [
    {"name": "format", "type": "choice", "default": "not stated", "range": "hex/base64/url-safe"},
    {"name": "length/preset", "type": "choice or number", "default": "common key sizes", "range": "API key / JWT / encryption key / session token presets"}
  ],
  "input_formats": ["settings only"],
  "output_formats": ["copyable text token"],
  "ux_patterns": ["preset buttons for common secret sizes", "format selector", "copy-focused output"],
  "limits": ["no clearly advertised hard cap in search result snippet"],
  "free_vs_paid": "free web tool"
}
```

### 2. ToolFarm OpenSSL Rand Generator — OpenSSL-compatible browser generator

```json
{
  "name": "OpenSSL Rand Generator",
  "url": "https://toolfarm.io/en/openssl-rand-generator",
  "features": [
    "uses browser cryptographic random bytes",
    "models the OpenSSL rand mental model",
    "hex and Base64 encodings",
    "explains byte count separately from rendered text length"
  ],
  "params_options": [
    {"name": "bytes", "type": "number", "default": "OpenSSL-like examples", "range": "not stated in search result"},
    {"name": "encoding", "type": "choice", "default": "hex/base64 oriented", "range": "hex or base64"}
  ],
  "input_formats": ["settings only"],
  "output_formats": ["text"],
  "ux_patterns": ["OpenSSL command equivalence", "encoding selector"],
  "limits": ["no CSV/JSON export evident from search result"],
  "free_vs_paid": "free web tool"
}
```

### 3. Encode64 Random Bytes Generator — security-token random bytes tool

```json
{
  "name": "Random Bytes Generator Online",
  "url": "https://encode64.com/en/security-token-tools/random-bytes-generator",
  "features": [
    "secure random byte generation",
    "hex, Base64 and Base64URL output",
    "use-case guidance for HMAC keys, salts, IVs, nonces, tokens and test fixtures"
  ],
  "params_options": [
    {"name": "encoding", "type": "choice", "default": "not stated", "range": "hex/base64/base64url"},
    {"name": "byte length", "type": "number", "default": "not stated", "range": "not stated"}
  ],
  "input_formats": ["settings only"],
  "output_formats": ["text"],
  "ux_patterns": ["security use-case examples", "format selector", "copyable output"],
  "limits": ["no deterministic seed mode evident from search result"],
  "free_vs_paid": "free web tool"
}
```

## Table stakes → in-model / out-of-model

| Table stake | Verdict | Where it lands |
| --- | --- | --- |
| Generate cryptographically secure random bytes locally | in-model | `getrandom` CSPRNG on CLI/WASI and browser crypto on web |
| Byte-count input, not character-count input | in-model | `bytes` parameter, 1-4096, summary prints bits |
| Multiple output encodings | in-model | `encoding` enum: hex, base64, base64url, binary, decimal, c-array, python-bytes |
| Hex and Base64 compatible with OpenSSL rand examples | in-model | text summary includes equivalent `openssl rand -hex/-base64 N` command |
| Base64URL for URL/JWT-style tokens | in-model | `base64url` enum, no padding |
| Common security presets | in-model | page example chips for AES-256 hex, JWT Base64, URL token, IV/MAC and code literals |
| Generate several values at once | in-model | `count` parameter, one value per line or JSON array |
| Byte separators for fingerprint/MAC-style hex | in-model | `separator` enum: auto, none, space, colon, dash, comma |
| Uppercase hex output | in-model | `uppercase` checkbox for hex and C-array encodings |
| JSON export for scripts/tests | in-model | `output=json` |
| Reproducible example/deep-link output | in-model | `seed_hex` deterministic mode, clearly warned as not secret unless seed is secret |
| Raw binary file download | out-of-model for this block | page emits copyable/downloadable text only; raw bytes can be reconstructed from hex |
| Server-side storage or secret vault integration | out-of-model | gizza tools are local transformations, not a vault/backend |
| Password phrase / pronounceable password UX | out-of-model | sibling password/token tools cover character and human-typed secrets |

## Decisions

- Defaults mirror common secure-key guidance: 32 bytes, one value, hex output, no seed.
- The descriptor uses enums for every fixed choice so the CLI schema, manifest and page selects stay aligned.
- Text output prints values first, then a compact entropy/encoding summary; JSON output exposes the same data programmatically.
- The cap is explicit: `bytes` up to 4096, `count` up to 100, and `bytes × count` up to 8192 per run.
- Seeded output exists only to support deterministic examples, page deep links and tests; the copy warns that seeded output is reproducible rather than fresh secret material.
