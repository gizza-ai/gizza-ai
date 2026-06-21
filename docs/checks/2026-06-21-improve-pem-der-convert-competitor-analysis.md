# pem-der-convert — competitor analysis (2026-06-21)

Tool: convert cryptographic objects (keys, certs, CSRs, CRLs) between **PEM**
(base64 text wrapped in `-----BEGIN <label>-----` armor) and **DER** (raw binary
ASN.1, shown/accepted as hex or base64) in either direction. Pure-Rust, runs
fully in-browser (page) / in-chat / via CLI.

## Surfaces verified

| Surface | Status | Evidence |
|---|---|---|
| Chat / LLM block | PASS | `wafer build` OK (338 KiB); drift-guard schema test passes |
| CLI | PASS | `gizza tool pem-der-convert` — auto (PEM→DER), auto (DER→PEM), explicit chain all correct |
| Page (query-params + fields) | PASS | Playwright `tool-page-pem-der-convert.spec.ts` — 2/2 (field-driven + deep-link) |

## Top competitors surveyed

1. **SSLShopper SSL Converter** (sslshopper.com) — file upload, converts between PEM/DER/PKCS#7/PKCS#12.
2. **monocalc PEM ↔ DER Converter** — in-browser, no upload; hex/base64 DER tabs; shows equivalent openssl command; auto-detects multiple `-----BEGIN----- blocks`.
3. **certificatetool.com / RapidSSL / LeaderSSL / Eunetic / HTTPCS** — server-side SSL converters across crt/cer/pem/der/p7b/pfx.
4. **wolfSSL / SSL.com guides** — documentation + openssl command references.
5. **GitHub gists (stuartpreston etc.)** — DER(base64)→PEM one-liners for Azure mgmt certs.

## Gap analysis (fit-to-model)

| Competitor capability | In gizza model? | Action |
|---|---|---|
| PEM → DER and DER → PEM both directions | yes | shipped (core both ways) |
| DER as **hex OR base64** (in and out) | yes | shipped (`der_format` param; hex accepts `0x`/`:`/`-`/whitespace) |
| **Auto-detect** PEM vs DER from the input | yes | **added** — `direction=auto` is the default; sniffs for `-----BEGIN` |
| Multiple `-----BEGIN----- blocks` (cert chains) → each block | yes | **added** — `pem_to_der_all` via `pem::parse_many`; output annotates `block N of M` |
| Shows the **detected PEM label** / object type | yes | shipped (`# label: <LABEL>` header + byte count) |
| Accepts a pasted armor line as the target label | yes | shipped (`normalise_label` extracts the label, uppercases, defaults CERTIFICATE) |
| In-browser, **nothing uploaded** | yes | shipped (pure-Rust wasm; no network) |
| Drag-and-drop **file** upload (.der/.crt binary file) | **no** | out of model — the pure page Input is text fields, not a binary file upload; users paste PEM text or hex/base64 DER. Noted, not built. |
| **PKCS#7 (.p7b) / PKCS#12 (.pfx)** container conversion | **no** | out of scope — those are different container formats requiring ASN.1 + (PFX) decryption, not a PEM↔DER re-encode. A separate tool. |
| Show the equivalent **openssl command** | partial | the output already names the label + format; a literal openssl cmd-line generator was judged low-value vs. clutter and not added (the tool *does* the conversion). |

## Copy / UX / visual

- Page H1 "PEM ⇄ DER Converter", SEO title/description/tags rewritten for the
  PEM↔DER / certificate-converter query space (all original copy — no competitor
  text, branding, or trademarks copied).
- `content.md` explains PEM-vs-DER, both directions, hex/base64, privacy, and
  input-format tips (separators, pasted armor labels, chain handling).
- Privacy ("nothing uploaded, runs in your browser") emphasised to match the
  in-browser competitor's main selling point.

## Notes

- Generic re-encoder by design: it does **not** parse the inner ASN.1, so it
  works for any object type (key/cert/CSR/CRL) rather than a fixed allow-list.
- No competitor copy, branding, or trademarks were reproduced.
