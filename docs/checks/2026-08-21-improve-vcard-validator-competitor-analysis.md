# vcard-validator — competitor analysis (2026-08-21)

Scan run **before** implementation, per the `/improve-tool` Phase 2–3 procedure applied to a
new build. All notes are **paraphrased observations of publicly documented behaviour** — no
competitor copy, branding, or trademarked text is reproduced or reused anywhere in this tool.

## Search

One web search: *"online vCard validator vcf file validate RFC 6350 check errors"*.
The result set was dominated by three genuinely comparable validators plus a
reference implementation. One candidate (`l0b0/vcard4`, a GitHub vCard-4 validator) was
**replaced**: the repository was archived in Oct 2022 and its own README states it is not
ready for use, so it is not a live competitor. It was swapped for **sabre/vobject**, the
widely-deployed PHP vCard library whose `validate()` API is the de-facto reference for what
a vCard validator is expected to check.

## Competitors skimmed

### 1. AnyOnlineTool — vCard Validator Online
- **Versions:** vCard 3.0 (RFC 2426) and 4.0 (RFC 6350), auto-detected from `VERSION`.
- **Checks:** missing required properties, invalid property syntax, incorrect line folding,
  encoding problems, general spec compliance, and value-level validation of dates, URLs and
  email addresses.
- **Input:** paste text or upload a `.vcf`; multiple cards in one file are validated
  separately and reported per card.
- **Output:** per-card status, a property count, and error detail with line numbers.
- **Positioning:** client-side execution, no upload.
- **Limits:** none published.

### 2. VirtualContactCards — vCard Validator
- **Versions:** 2.1, 3.0 and 4.0.
- **Checks:** structural integrity (`BEGIN`/`END` delimiters), presence of `VERSION`,
  presence of `FN`, recognition of the common standard properties (`EMAIL`, `TEL`, `ORG`,
  `URL`, `TITLE`), and parameter handling on forms like `TEL;TYPE=WORK`.
- **Output:** the parsed fields, hard errors, and separately a set of *compatibility*
  warnings — i.e. a two-tier severity model rather than pass/fail.
- **Options:** none exposed; upload or paste, then validate.
- **Limits:** none published.

### 3. sabre/vobject — `VCard::validate()` (reference implementation)
- **Severity model:** three levels — (1) a problem that was detected and auto-repaired,
  (2) valid but likely to cause interoperability trouble, (3) an invalid document.
- **Options:** a `REPAIR` bitfield that mutates the object toward validity, and a
  `PROFILE_CARDDAV` bitfield that enforces a stricter profile (notably requiring `UID`).
- **Scope caveat:** the library documents its parser as doing *basic* syntax checking and
  explicitly does not claim exhaustive value validation.
- **Versions:** parses/converts between 2.1, 3.0 and 4.0, and documents that conversion is
  lossy in places (e.g. 2.1's `AGENT` is dropped).

## Table-stakes extracted → in-model / out-of-model

| Capability | Seen in | Verdict | Where it lands |
| --- | --- | --- | --- |
| Auto-detect version from `VERSION`, support 2.1 / 3.0 / 4.0 | all three | **in-model** | `version = auto` default + explicit override |
| Enforce version-specific required properties (`FN` in 3.0/4.0, `N` in 2.1/3.0) | 1, 2, sabre | **in-model** | `missing-fn`, `missing-n` rules |
| `BEGIN`/`END` structural integrity, stray content outside a card | 2, sabre | **in-model** | `unclosed-card`, `stray-end`, `content-outside-card` |
| `VERSION` present, and first property after `BEGIN` in 4.0 | 1, 2, sabre | **in-model** | `missing-version`, `version-not-first`, `unknown-version` |
| Two-tier severity (error vs interoperability warning) | 2, sabre | **in-model** | every issue carries `error`/`warning` |
| Per-card reporting for multi-card files | 1, 2 | **in-model** | report groups by card; JSON carries a `card` index |
| Line numbers on every issue | 1 | **in-model** | 1-indexed physical source line |
| Line-folding / long-line checks | 1, 2 | **in-model** | `long-line` (>75 octets unfolded), `stray-fold` |
| Property-syntax checks (name charset, missing `:`, group prefix) | 1, 2 | **in-model** | `missing-colon`, `invalid-property-name`, `empty-property-name` |
| Parameter handling (`TEL;TYPE=WORK`), bare 2.1-style params | 2, sabre | **in-model** | `bare-parameter` (legal in 2.1, error in 3.0/4.0), `unquoted-parameter` |
| Value validation for **email** | 1 | **in-model** | `invalid-email`, toggleable |
| Value validation for **phone numbers** | implied by the backlog brief; none of the three do it properly | **in-model, differentiator** | `invalid-tel` via the `phonenumber` crate + `default_country` region hint |
| Value validation for **dates** (`BDAY`/`ANNIVERSARY`/`REV`) | 1 | **in-model** | `invalid-date` (version-aware: 4.0 allows partial dates) |
| Value validation for **URIs** (`URL`, `SOURCE`, …) | 1 | **in-model** | `invalid-uri` |
| Structured-value arity (`N` = 5 components, `ADR` = 7) | sabre (via property classes) | **in-model** | `invalid-n`, `invalid-adr` |
| Enumerated values (`KIND`, `GENDER`) | sabre | **in-model** | `invalid-kind`, `invalid-gender` |
| Cardinality (`VERSION`/`N`/`BDAY`/`UID`/… at most once) | sabre | **in-model** | `duplicate-property` |
| Non-standard property detection | 1 ("spec compliance") | **in-model** | `unknown-property` (warning; `X-` exempt) |
| Stricter deployment profile (CardDAV: require `UID`) | sabre | **in-model, generalised** | `required_properties` free-list — `UID` is just one value a user can pass |
| Machine-readable output for CI | none of the three | **in-model, differentiator** | `output = json` |
| **Auto-repair** the document | sabre (`REPAIR`) | **considered, rejected** | Out of scope for a *validator*: this repo already ships `vcard-normalize` (E.164 phones, email casing, name tidying) as the repair surface. Duplicating it here would fork the fixer logic across two tools. Noted on the page instead. |
| **File upload** of a `.vcf` | 1, 2 | **considered, rejected** | The page is a paste-text widget; `.vcf` is plain text, so paste covers it. A file picker would need a new generic input kind, not a per-tool hack. |
| **Encoding / charset detection** (`CHARSET=`, quoted-printable) | 1 | **out-of-model (partial)** | Input reaches the block already decoded as UTF-8 by the page/CLI, so byte-level charset detection can't be done here. The tool *does* flag a `CHARSET` parameter as a 2.1-only construct. `text-encoding-converter` is the tool for the byte-level job. |
| **Version conversion** (2.1 → 4.0) | sabre | **out-of-model for this tool** | A converter is a separate tool shape, not a validator option. |
| Multi-language UI | 1 | **out-of-model** | This repo renders one generic English page; localisation is a site-repo concern. |

## Design decisions taken into the descriptor

1. `version` is an **enum** (`auto`/`2.1`/`3.0`/`4.0`), default `auto` — matching every
   competitor's auto-detect while adding the explicit "I am targeting 4.0, tell me what
   breaks" mode none of them expose.
2. Severity is **two-tier** (error/warning), following VirtualContactCards and sabre levels
   2–3. sabre's level 1 ("repaired") has no analogue because this tool never rewrites.
3. Phone validation is the deliberate differentiator: `check_phone` + `default_country`
   (ISO-3166 alpha-2), reusing the `phonenumber` crate already proven wasm-safe by the
   sibling `vcard-normalize` block.
4. `required_properties` generalises sabre's `PROFILE_CARDDAV`: instead of one hard-coded
   profile, the user names the properties their target system needs.
5. `output = json` gives the CI surface no scanned competitor offers.
6. Every rule has a stable `rule` slug so JSON output is greppable/filterable.

## Not a duplicate of an existing block

`ls blocks/ | grep -i vcard` → `vcard-normalize`, `vcard-to-json`, `vcard-deduplicate`,
`csv-to-vcard`. Their `core/src/lib.rs` were read: all three vCard siblings **transform**
(rewrite values / re-serialise as JSON / merge duplicate cards) and none of them diagnose or
report. `vcard-validator` produces a read-only issue report and never emits a card — a
distinct output shape, so it is not skiplisted.
