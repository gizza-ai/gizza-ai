# shamir-secret-recover competitor analysis — 2026-08-18

Backlog item: `shamir-secret-recover` — reconstruct a secret from a threshold of Shamir shares
(Lagrange interpolation at x = 0 over a finite field), pure compute, offline.

Scope note: the backlog row is the **recover / combine half only** ("Reconstructs a secret from a
threshold of Shamir shares", use case "recover the secret from these 3 shares"). Every competitor
below bundles split + combine in one page; this block is the combine side, and the split side stays
out of scope for the reason recorded under "Deliberately not built".

## Sources skimmed

One WebSearch for the function ("Shamir secret sharing online tool combine shares recover
secret"), then the top hits were inspected directly. Two of the ranked hits could not be read
(`variedtools.com` returns HTTP 403 to a non-browser client, `liminfo.com` serves a certificate for
an unrelated host), so they were replaced by the next real tools in the ranking. All observations
below are paraphrased — no competitor copy, naming or branding is reused anywhere in this block.

| Competitor | What it exposes | Table-stakes patterns observed | Fit decision |
| --- | --- | --- | --- |
| DevGlan — Shamir's Secret Sharing (split & recover) | Two panels: split (secret, encoding selector UTF-8 / HEX / Binary, "Total Shares (N)", "Required Shares (K)") and recover (paste shares, pick the same encoding, get "Recovered Secret" with a copy button). Shares render as a bracketed `[index-value]` pair, e.g. `[1-8caff120934bd99a]`, where the first field is the x coordinate and the second is the hex y value. TXT/PDF download of the generated shares. Suggests preset splits such as 5-of-3 and 6-of-3. FAQ covers "nothing is stored server-side", crypto/seed-phrase use, and that the encoding choice does not change security. | Index-prefixed hex share format; explicit threshold (K) field; an output-encoding selector covering text, hex and binary; copy button; statement that fewer than K shares cannot recover anything. | **In-model.** `share_format = index-prefix` parses exactly this, brackets and all; `secret_encoding` covers `text`/`hex`/`binary`; `threshold` is an explicit optional param. Out-of-model: PDF/TXT share downloads (a split-side artifact; the page already gets a generic text download + copy button from the platform). |
| Encrypt-Online — Shamir Secret Sharing | Split section (secret, "required shares" threshold, "total shares") and a separate combine section where shares are pasted **one per line** into a textarea and a "Recovered secret" is shown. Explicit safety notice about storing shares securely and an honest caveat that the maths does not protect against bad storage, a compromised page or an implementation defect. Offers CLI install instructions and a Node library as alternatives. | One-share-per-line textarea as the canonical combine input; separate threshold field; prominent limits/safety copy; a CLI equivalent for scripted use. | **In-model.** `shares` is a multiline textarea parsed one share per line (blank lines and `#` comments ignored, separators `,`/`;` also accepted); the page states the same honest limits; the generated CLI example is the scripted equivalent. |
| Elysia Tools — Shamir Secret Sharing | Split (secret, threshold `k`, total `n`, with `2 ≤ k ≤ n ≤ 255`) and combine modes. Shares are `sss:`-prefixed base64url strings whose decoded bytes are the x coordinate followed by the y bytes. Documents the scheme as byte-wise GF(256) with a degree k−1 polynomial per byte and Lagrange interpolation for recovery. Explicitly lists limitations: **no integrity checking** (a modified share silently yields a wrong secret), max 255 shares, minimum threshold 2, nothing leaves the browser. Worked scenarios for a 3-of-5 shared password and a 2-of-3 wallet recovery. | Prefixed base64url share strings with a **leading** index byte; GF(256) byte-wise field documented; hard bounds 2 ≤ k ≤ n ≤ 255; an admitted tamper-detection gap; scenario-style worked examples. | **In-model.** `share_format = leading-index` plus base64/base64url decoding handles this format including the `sss:` prefix; bounds are enforced and stated; the admitted tamper-detection gap is closed by the cross-check described below. |
| Reference implementations consulted for format coverage (not a UI competitor): `ssss` (point-at-infinity, the Unix CLI), HashiCorp Vault's unseal shares, `secrets.js` | `ssss-combine` reads `index-hex` shares over GF(2^n) with n = the security level (8–1024 bits) plus a diffusion layer; Vault-style shares are raw bytes whose **last** byte is the x coordinate, printed as hex or base64; `secrets.js` packs the field bit-size and the id into a hex header. | Three more real-world share layouts a "recover" tool is asked to eat: trailing-index raw bytes, index-prefixed hex, and header-packed hex. | **Partly in-model.** `share_format = trailing-index` covers the Vault-style layout. `ssss`'s own format and `secrets.js`'s packed header are out of model — see "Deliberately not built". |

## Descriptor decisions

Every table-stake above lands in the descriptor or in the out-of-model list below; none was dropped
silently.

- **`shares`** — required multiline string, one share per line. Blank lines, `#` comments, surrounding
  `[...]`, quotes, and `,`/`;` separators are tolerated because users paste from three different
  competitors' output shapes. Caps: at most 255 shares (the GF(256) x-coordinate limit every
  competitor states) and 65,536 bytes of secret per share.
- **`share_format`** — `Param::enumv` over `auto` (default), `index-prefix`, `leading-index`,
  `trailing-index`. This is the union of the three real layouts observed (DevGlan's `1-hex`,
  Elysia's `sss:` base64url with a leading index byte, Vault's trailing index byte). `auto` is the
  default because a recover-only tool cannot assume which splitter produced the shares.
- **`share_encoding`** — `Param::enumv` over `auto` (default), `hex`, `base64`. Base64 detection
  accepts both standard and URL-safe alphabets with or without `=` padding. Explicit override exists
  because an all-hex payload is also legal base64, so `auto` prefers hex on that overlap.
- **`field_poly`** — `Param::enumv` over `auto` (default), `0x11b`, `0x11d`. Competitors say
  "GF(256)" without naming the reduction polynomial, and the two in real use disagree: `0x11b` is the
  AES polynomial (Vault-style implementations) and `0x11d` is what `secrets.js`-lineage code uses.
  Picking the wrong one returns a plausible-looking but wrong secret, so `auto` resolves it from the
  shares themselves whenever there is at least one spare share.
- **`threshold`** — optional integer (0 = unset, meaning "use every share supplied"), 0 or 2–255.
  Mirrors the K field every competitor exposes, and additionally enables exact
  "which share disagrees" reporting when more than K shares are supplied.
- **`verify`** — boolean, default true. This is the **capability gap** the competitor set openly
  admits ("no integrity checking — modified shares produce incorrect secrets without detection").
  When there is redundancy (more shares than the threshold) the block reconstructs from several
  different share subsets and reports whether they agree, which detects a corrupted or foreign share
  instead of silently returning garbage.
- **`secret_encoding`** — `Param::enumv` over `auto` (default), `text`, `hex`, `base64`, `binary`.
  Covers DevGlan's UTF-8 / HEX / Binary selector plus base64 for key material; `auto` prints text
  when the bytes are valid printable UTF-8 and hex otherwise, so a binary key never renders as
  mojibake.
- **`output`** — `Param::enumv` over `secret` (default), `report`, `json`. `secret` matches the
  competitors' single "Recovered secret" box; `report` explains which format/field/shares were used
  and what verification concluded; `json` is the scriptable shape.

## UX controls carried over

- Multiline paste box for the shares (`multiline = true`), matching the one-share-per-line textarea
  every combine panel uses.
- Friendly `<select>` labels via `[input.labels]` for the format / encoding / field enums, since the
  canonical values (`0x11b`, `leading-index`) are jargon.
- `[[example]]` preset chips instead of the competitors' prose "try 3-of-5" suggestions: one chip per
  real share layout plus a tamper-detection chip, each prefilled with runnable shares.
- Copy result and Reset come from the platform; a text download comes free with `format = "text"`.

## Verification matrix to cover

- One real run per `share_format` value (`auto`, `index-prefix`, `leading-index`, `trailing-index`).
- One real run per `share_encoding` value and per `secret_encoding` value, including the hex/base64
  overlap case.
- Both `field_poly` values plus `auto` correctly resolving a `0x11d` share set.
- `verify` in its non-default (unchecked) state, and a corrupted-share run that reports `failed`.
- Threshold supplied vs unset; fewer shares than the threshold (error); duplicate x (error);
  mismatched share lengths (error); a single share (error).
- Cap boundary: 255 shares accepted, 256 rejected.
- Cross-check against an independent Python GF(256) implementation, and the AES field test vector
  0x57 · 0x83 = 0xc1 for the `0x11b` multiplier.

## Deliberately not built

- **Splitting / share generation.** The backlog row is the recover half, and splitting needs a
  cryptographically secure RNG whose availability differs per surface. It belongs in its own block so
  that this page stays a safe read-only operation over material the user already holds.
- **`ssss` (point-at-infinity) native shares.** That tool works over GF(2^n) with n equal to the
  chosen security level (8–1024 bits, arbitrary-precision arithmetic) and adds a diffusion layer, so
  its shares are not byte-wise GF(256) points. Supporting it means a bignum field plus an exact
  reimplementation of the diffusion step with no reference binary available here to verify against —
  listed rather than half-built, and the page says so.
- **`secrets.js` packed-header shares** (field bit-size and id encoded in a hex header, with
  configurable 3–20 bit fields). Same reason: a different field size per share, not GF(256).
- **SLIP-39 / BIP39 mnemonic shares.** These wrap Shamir in a 1024-word wordlist, a checksum, an
  encryption layer and PBKDF2 iterations; that is a distinct tool, not a parameter of this one.
- **Prime-field (integer mod p) share sets.** Some teaching demos use arithmetic modulo a prime
  rather than GF(256); the share strings are decimal and unambiguous to spot, and the page names this
  as unsupported.
- **PDF/TXT share export, share-distribution helpers, QR codes for shares.** Split-side product
  chrome; the page already offers copy and text download generically.
- **Server-side or account features** (storing shares, emailing them to holders): out of the
  browser-local, no-account model entirely.
