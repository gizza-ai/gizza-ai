# pgp-verify — competitor analysis & improvement snapshot (2026-06-22)

Tool: `blocks/pgp-verify` — verify an OpenPGP (PGP/GPG) signature against a
message and the signer's public key. Pure-Rust (rPGP), runs on all surfaces
(chat block, CLI, browser page). No network.

## Surfaces verified (Phase 1)

- **Chat block**: `wafer build` validates/instantiates `target/block.wasm` (1634 KiB).
- **CLI**: generated a key + signatures with the existing `generate-pgp-key-pair`
  and `pgp-sign` tools, then verified via `gizza tool pgp-verify`:
  - detached valid → `valid:true`, fingerprint matches the signing key
  - detached tampered message → `valid:false` with a clear error
  - clearsigned valid → `valid:true`, embedded `signed_text` exposed
- **Page**: 3 Playwright tests (`tests/tool-page-pgp-verify.spec.ts`) — valid
  detached, tampered detached, valid clearsign — all pass.
- **Unit**: 9 core/block tests (happy + error paths, JSON shape, drift guard).

## Top 5 competitors surveyed

1. **PGP Tool (pgptool.dev/verify)** — paste clearsigned message + public key.
   Reports valid/invalid, signer identity, fingerprint, signature timestamp,
   detailed failure messages. Optional key auto-retrieval (network). No batch.
2. **8gwifi.org (/pgpfileverify.jsp)** — signed file upload + pasted public key;
   attached/clearsigned. Reports only a pass/fail "valid" — no fingerprint, date,
   or UID. Built-in example.
3. **Toolsley (toolsley.com/verify.html)** — three files (document, signature,
   public key) for detached verification. Strongest UX: drag/drop, keyserver
   lookup + auto-fetch key by key ID, shareable verify link. In-browser.
4. **onlinepgp.com** — pasted message + public key; generic "verification result"
   with no detailed metadata. Import-key + copy buttons, no-storage promise.
5. **browserPGP (browserpgp.github.io/verify.html)** — paste signed message +
   public key, minimal output panel, no documented metadata fields. Paste-only.

**Cross-tool observation:** most verifiers surface only pass/fail; few report
the signer's UID/email, fingerprint, signing time, or hash algorithm, and
auto-detecting detached vs clearsigned is rare (several use separate tools/modes).

## Gap diff & what was closed (fit-to-model)

### In-model gaps — addressed
- **Auto-detect detached vs clearsigned** (rare elsewhere) — done: the shape is
  detected from the armor header; no mode parameter needed.
- **Report signer User ID (name + email)** — done: `signer_user_id` from the
  key's primary UID (almost no competitor surfaces this).
- **Report signature creation time** — done: `signed_at` (RFC 3339 UTC).
- **Report fingerprint + key ID on every result** — done: `signer_fingerprint`,
  `signer_key_id`.
- **Report hash algorithm** — done: `hash_algorithm` (canonical name, e.g.
  `SHA256`), added during this pass.
- **Echo the verified message body** — done for clearsign: `signed_text`.
- **Precise failure copy** — done: distinct error strings for malformed armor,
  missing key, wrong key / tampered content; page copy distinguishes "signature
  math valid" from "you trust the signer" and advises checking the fingerprint.
- **Verify against primary key AND every subkey** — done, so subkey-signed
  messages verify.

### Out-of-model gaps (not built — need network or out of scope)
- Keyserver / WKD lookup or auto-fetch of the signer's key by key ID or email
  (Toolsley-style) — requires network; this tool is offline by design.
- Web-of-trust / trust-path evaluation and keyserver revocation checking —
  requires network + trust DB.
- Multi-file batch verification — the page/CLI take a single message.
- **Key-expiry status** — left out this pass: meaningful expiry reporting needs
  to walk the key's binding self-signatures for expiry subpackets and compare to
  a clock the page (wasm32-unknown-unknown) lacks a std source for; deferred to
  avoid a half-correct signal. (Signature validity itself is unaffected.)

## Output shape

`{valid, mode, signer_key_id, signer_fingerprint, signer_user_id, signed_at,
hash_algorithm, signed_text?, error?}` — `valid` is the gate; metadata fields are
omitted when absent; `error` is present only on failure.
