# iban-validator — competitor analysis (2026-06-22)

## Tool
`iban-validator` — validate an IBAN (ISO 13616) via the ISO 7064 mod-97 checksum,
country-specific length check, and parse out the country, check digits, BBAN, and
(for common countries) bank / branch / account components. Pure Rust, runs on all
surfaces (chat skill, CLI, in-browser page). Nothing is uploaded.

## Top competitors surveyed
1. **iban.com** (iban-checker) — supports 87 IBAN countries (36 SEPA); validates
   bank/branch/account/check digits per country; identifies the BIC; runs national
   domestic check-digit algorithms for 36 countries.
2. **bank.codes** — breaks the IBAN into country, BBAN, local bank code, branch
   code and account number; SEPA-wide.
3. **EaseCloud IBAN Validator & Parser** — country code, check digits, bank
   identifier (BBAN), account components; 70+ countries.
4. **FastTool IBAN Validator** — in-browser, offline, 75+ countries.
5. **openiban.com** — checksum + country length validation; public web service.

Sources:
- https://www.iban.com/iban-checker
- https://bank.codes/iban/validate/
- https://www.easecloud.io/tools/misc/iban-validator-parser/
- https://fasttool.app/tools/iban-validator
- https://openiban.com/

## Feature diff

| Capability | Competitors | iban-validator | Notes |
|---|---|---|---|
| mod-97 (ISO 7064) checksum | yes | yes | core gate |
| Country code + name | yes | yes | 75-country registry |
| Country-specific length check | yes | yes | flags right-checksum/wrong-length |
| Check digits surfaced | yes | yes | |
| BBAN extraction | yes | yes | |
| Bank code parse | yes | yes (10 common countries) | GB/IE/DE/FR/MC/ES/NL/IT/SM/BE/CH/LI/AT |
| Branch code parse | yes | **yes (added this pass)** | GB/IE/FR/MC/ES/IT/SM |
| Account number parse | yes | yes (common countries) | |
| 4-block formatted display | some | yes | |
| Space/case normalization | yes | yes | |
| In-browser / offline | some (FastTool) | yes | nothing uploaded |
| Chat + API + CLI surfaces | no | yes | unique to gizza |

## Gaps closed this pass
- **Branch (sort) code extraction** — added a `branch_code` field, parsed for the
  countries whose BBAN structure has a distinct branch segment (GB/IE sort code,
  FR/MC branch, ES branch, IT/SM CAB). Surfaced in chat JSON, CLI JSON and the page
  output. This matches bank.codes / iban.com which display the branch code.

## In-model gaps deliberately NOT built
- **BIC / bank-name lookup** (iban.com): requires a maintained bank-directory
  dataset or a live registry call — out of scope for a pure, offline, no-network
  tool. We surface the bank *code*, not the named bank.
- **National domestic check-digit algorithms** (iban.com runs these for 36
  countries): each is a separate per-country algorithm + lookup table; large and
  data-heavy. The ISO 7064 mod-97 checksum already catches all single-digit and
  most transposition typos, which is the primary validation purpose.
- **IBAN generation** (some sites bundle a generator): that's a distinct tool, not
  validation.

## Verification (all green)
- `cargo test --workspace` — 9 tests pass (8 core + 1 schema drift-guard).
- `wafer build` — chat block instantiates (302 KiB).
- CLI: `gizza tool iban-validator iban="…"` — valid GB/DE/FR and invalid checksum
  all return correct JSON incl. bank/branch/account.
- Page: Playwright `tool-page-iban-validator.spec.ts` — 2 tests pass (valid UK IBAN
  shows country + bank code; bad-checksum deep-link shows INVALID).

No competitor copy, branding, or trademarks were used.
