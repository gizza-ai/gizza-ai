# check-digit-validator competitor analysis (2026-08-16)

Tool: `check-digit-validator` — validates (or computes) the check digit of a code against the common schemes: Luhn/mod-10, the GS1 mod-10 family, mod-11, IBAN mod-97-10, ABA, ISIN and VIN.

## Sources scanned

- EAN Check (`eancheck.com`) — paste-a-code identifier for GTIN-8/12/13/14; validates and appends check digits, verified batches of over a million codes, all client-side, no step display, no export.
- Calculator.now check digit calculator (`calculator.now/check-digit-calculator`) — UPC/GTIN-12, EAN/GTIN-13, ISBN-10/13, ISSN, credit card, NPI, IMEI, VIN, generic mod-10 (Luhn) and generic mod-11; calculate/verify dropdown, optional "show calculation steps", single code at a time, no IBAN.
- GS1 official calculators (`gs1.org/services/check-digit-calculator`, GS1 US, GS1 Germany's `checkDigitCalculator`) — authoritative mod-10 calculators for GS1 keys only (GTIN-8/12/13/14, SSCC, GLN); compute-oriented, single key, no other scheme families.
- Bulk-oriented calculators (`morovia.com/bulk-check-digit-calculation`, `limeconvert.com/barcode-calculator`) — paste a list and get check digits back for EAN-13/8, ISBN, GTIN, SSCC-18, ITF-14; barcode families only, compute-first.
- Single-scheme specialists (`simplycalc.com/luhn-calculate.php` for Luhn, `freeisbn.com/check-digit` for ISBN-10/13, IBAN validators such as `check.town`) — deep on one scheme, with format auto-detection inside that one family.
- Aggregator calculators (`allcalculators.co.uk`, `calculators.sg`, `neocalculators.com`, `tooldone.com`) — multi-scheme pages built around a scheme dropdown; several advertise an optional step-by-step display.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | In current gizza model? | Decision |
| --- | --- | --- | --- |
| Validate an existing check digit | Every tool scanned | Yes | Default `mode=validate`; reports VALID/INVALID per code. |
| Compute a missing check digit | GS1 calculators, bulk tools, Calculator.now | Yes | `mode=compute` returns the digit plus the completed code. |
| Luhn / generic mod-10 | Luhn specialists, aggregators | Yes | `luhn`, plus `credit-card`, `imei`, `npi` as length-constrained variants. |
| Card brand identification | Card-focused validators | Yes | `credit-card` reports the brand (Visa, Mastercard, Amex, …) in `detail`. |
| GS1 family (EAN-8/13, UPC-A, GTIN-14, SSCC) | GS1 official, EAN Check, bulk tools | Yes | All five implemented on one shared ×3/×1 weighting. |
| ISBN-10 / ISBN-13 / ISSN incl. `X` | ISBN specialists, aggregators | Yes | mod-11 with `X` handled for ISBN-10 and ISSN. |
| IBAN mod-97-10 with country length check | IBAN specialists; absent from the multi-scheme calculators | Yes | Implemented with per-country length table and country name in `detail`; a right-check-digit/wrong-length IBAN is still reported invalid. |
| ABA routing number (3-7-1) | A few finance aggregators | Yes | Implemented; reports the Federal Reserve routing symbol. |
| ISIN | Rare outside finance-specific tools | Yes | Letter expansion then Luhn. |
| VIN check character | Calculator.now, VIN specialists | Yes | mod-11 transliteration at position 9; compute mode rewrites that position rather than appending. |
| Scheme auto-detection | Within one family only (EAN Check across GTIN lengths; ISBN tools across 10/13) | Yes — and wider | `scheme=auto` tests every scheme the length and character set allow across all families, and lists every scheme the code also passes under `also valid as:`. Differentiator: no scanned competitor detects across families. |
| Show the arithmetic | Calculator.now, Calculators.sg | Yes | `show_steps=true` prints the weighted sum and modulo step per scheme, including the mod-97 and transliteration steps that competitors' step displays omit. |
| Batch input | EAN Check (barcodes only), Morovia, LimeConvert | Yes | Newline/comma/semicolon separated, 5,000 per run, with a valid/invalid/unreadable tally. Cap is lower than EAN Check's million but covers a spreadsheet column and keeps the wasm run bounded. |
| Separator tolerance | Mixed; several require bare digits | Yes | Spaces, dashes, dots, slashes and colons stripped before checking, so invoice/label copy-paste works. |
| Explains the expected digit on failure | Uncommon — most return only OK/invalid | Yes | Prints `expected check digit X, got Y`, which localises the typo. Differentiator. |
| Client-side only | EAN Check states it explicitly; most others unclear | Yes | wasm in the browser; stated on the page. |
| Barcode image rendering | LimeConvert and some barcode sites | Out-of-model | Not built; this is a check-digit tool, not a barcode generator. |
| GLN / ITF-14 / GS1-128 / Digital Link parsing | EAN Check | Partially out-of-model | GLN is a 13-digit GS1 key and already validates as `ean13`; AI-element-string parsing is a separate tool's job. |
| CSV/file upload and export | Bulk tools offer file in/out | Out-of-model for this page | Paste-in, text-out; the CLI surface covers scripted use and the JSON response carries per-item fields for automation. |
| Live issuer/bank lookup (BIN, IBAN bank name, VIN decode) | Some paid validators | Out-of-model | Requires a network database; this tool stays local and pure. Local context (brand, country, routing symbol, WMI) is included instead. |

## Defaults and examples chosen

- `scheme=auto` is the default because cross-family detection is the tool's differentiator: paste anything and it identifies what it is before judging it.
- `mode=validate` is the default because checking a code that already exists is the more common task; compute mode requires an explicit scheme since auto-detection reads the digit compute mode produces.
- `show_steps=false` keeps the first result readable; the arithmetic is one checkbox away for teaching or debugging.
- Example chips cover a payment card (auto-detect + brand), an ISBN-13 typo (the expected-digit message), a mixed batch (card + book + IBAN + routing), ISBN-13 compute, IBAN compute from country + account, and a VIN with steps shown — one chip per capability the competitor scan flagged as table stakes.
- Placeholder shows three codes from different families on separate lines so the batch and separator-tolerance behaviour is visible before anything is typed.

## Copy and UX notes

- All copy is generic and brand-free; no competitor wording, naming or layout was reproduced.
- The page states plainly that a valid check digit catches typos but does not prove a code was issued — a caveat most competitors leave implicit.
- The FAQ answers the questions the competitor scan showed users hitting: validate vs compute, why one code matches several schemes, whether validity means the code is real, why ISBN-10/ISSN can end in `X`, batch limits, and whether pasting a real card number is safe.
