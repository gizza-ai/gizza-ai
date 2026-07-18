# iban-extractor-validator — competitor analysis (2026-07-18)

Scope: a tool that scans **free-form text** for IBANs and validates each with the
ISO 13616 mod-97 checksum. Distinct from the existing single-IBAN
`blocks/iban-validator` (which validates one IBAN you already isolated) — this is
the *extract-from-text* member of the family (cf. `extract-ip-addresses`,
`extract-email-addresses`), plus per-match validation.

Sources skimmed (paraphrased; no copy/branding reproduced):

- **iban.com IBAN Checker** — single-IBAN validation: country code, length,
  mod-97 check digits; bank/BIC identification as a paid/registry feature.
- **IBAN.Guide (bulk)** — validates many IBANs at once, drag-and-drop CSV/TXT,
  auto-extracts and validates all IBANs in a file. Closest analogue to this tool.
- **Online Tools Forge / EaseCloud IBAN Validator & Parser** — validate
  structure, length, checksum for 100+ countries; parse out bank code, branch
  code, account number.
- **openiban.com** — public validation + calculation webservice; also does bank
  lookup via a country bank registry.
- **workdaten.eu / lingoservice IBAN validator** — MOD-97 check, country
  detection, bank-code extraction, 4-block formatting.

## Table-stakes → decision

| Capability | In-model? | Decision |
|---|---|---|
| Find every IBAN in pasted text (spaced 4-blocks **and** contiguous) | yes | **Built** — anchor on country code + 2 check digits, read to the country's registered length, tolerate grouping spaces. |
| ISO 13616 mod-97 (ISO 7064) checksum per IBAN | yes | **Built** — reuses `iban-validator` core `validate()` (single source of truth). |
| Country-specific length check | yes | **Built** — per-country length drives how many chars each candidate consumes (`expected_length` helper added to `iban-validator` core). |
| Country detection + name | yes | **Built** — returned per valid IBAN. |
| 4-block display formatting | yes | **Built** — `formatted` field. |
| Parse bank code / account number from BBAN | yes | **Built** — for common countries, via the reused core's `split_bban`. |
| Deduplicate repeated IBANs | yes | **Built** — first-seen order, spaced/unspaced forms collapse to one. |
| Flag near-miss typos (right length, bad checksum) separately | yes | **Built** — `invalid` list with reason "failed the mod-97 checksum". |
| Valid/invalid counts / summary | yes | **Built** — `count`, `valid_count`, `invalid_count`. |
| Bulk / many IBANs at once | yes | **Built** — a paste of any size is scanned in one run. |
| **Bank name / BIC lookup** | no | **Out-of-model** — needs an external bank-directory / BIC registry (megabytes of data + updates). Listed on the page as a stated limit, not built. |
| **Live account-existence / bank-reachability check** | no | **Out-of-model** — needs a bank API; documented as a limit ("checksum, not existence"). |
| **CSV/TXT file upload** | partial | Page uses a multiline paste box (`multiline = true`), which covers pasted invoices/statements/CSV text. A dedicated file-picker for this pure text tool is unnecessary — paste handles it. |

## UX / controls

- Multiline paste field (`multiline = true`) so newlines in pasted statements
  survive — matches the extract-* family.
- Two `[[example]]` preset chips (an invoice snippet with two valid IBANs; a
  "spot the typo" snippet mixing a valid and a checksum-broken IBAN) — the
  declarative preset answer, doubling as the page's worked examples.
- Text output: a summary line + a Valid list (with country) + an Invalid list
  (with reason); Copy-result + Reset buttons come free from the generator.

## Notes

- No competitor copy, branding, or trademarks were reproduced.
- Out-of-model items (bank-name/BIC lookup, live account check) are listed on the
  page's "Limits & edge cases" section, not implemented.
