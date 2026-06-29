# pgp-key-info — competitor analysis & surface checks (2026-06-29)

**Tool:** `pgp-key-info` — inspect an ASCII-armored OpenPGP public or private key
locally and report fingerprint, key ID, algorithm, dates, user IDs, and subkeys.

## Surface checks

| Surface | Check | Result |
| --- | --- | --- |
| Core/block | `cargo test --workspace` | ✅ unit + drift guard passed |
| Chat block | `wafer build` from `blocks/pgp-key-info/` | ✅ block wasm built |
| Web wasm | `wasm-pack build blocks/pgp-key-info/web --target web --release --out-dir pkg` | ✅ web pkg built |
| Wafer fixture | `wafer test` | ✅ public-key fixture passed |
| Page generator | `cargo run --manifest-path tools/generator/Cargo.toml -- .` | ✅ rendered `tools/pgp-key-info/` |
| CLI | `gizza tool pgp-key-info key=...` | ✅ returned expected JSON metadata |
| Page | `tool-page-pgp-key-info.spec.ts` | ✅ 2 passed (happy path + query-param deep-link) |

## Competitor scan (paraphrased)

1. **GnuPG / `gpg --show-keys --with-fingerprint`** — trusted local CLI view with
   fingerprint, key IDs, user IDs, subkeys, creation, expiry, and capabilities.
2. **OpenPGP.js key inspection examples** — browser-capable parsing that exposes
   fingerprints, user IDs, algorithms, and key/subkey metadata for JavaScript apps.
3. **Keyoxide / WKD-style key viewers** — public-key profile pages emphasize
   fingerprint verification, user identities, and proof/account context.
4. **Mailvelope key manager** — browser UI surfaces key user IDs, fingerprint,
   key validity, and whether keys can encrypt or sign.
5. **CyberChef PGP operations** — privacy-oriented browser tooling around PGP
   blocks; users expect paste-in/paste-out workflows and clear error messages.

## Gap decisions

- **Built:** full fingerprint and key ID display, public/private armored key parsing,
  user IDs, dates, primary algorithm, subkey list, and sign/encrypt capabilities.
- **Built:** multiline paste field, JSON output for copyability, query-param deep-link
  test, and local-only browser execution.
- **Out of model:** live keyserver/WKD fetching, trust signatures, revocation status
  from remote sources, and account/profile verification require network/server context
  and are intentionally not included.

All copy here is original and paraphrased; no competitor copy, branding, or assets were copied.
