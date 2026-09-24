# Competitor analysis: digit-to-words

Date: 2026-09-24
Tool: `digit-to-words`

## Comparator scan

Reviewed the common shape of public number-to-words tools before implementation: calculator-style converters, check-writing amount converters, and simple browser text converters. The relevant competitors converge on the same table-stakes controls rather than on a proprietary workflow:

| Competitor shape | Table-stakes behavior observed | In model? | Implemented decision |
| --- | --- | --- | --- |
| General number-to-words converter | Paste a number and return cardinal English words; accept grouping separators and decimals. | Yes | Default `cardinal` style spells exact digit strings, accepts common separators, comma decimal point, signs, accounting parentheses, currency symbols, and scientific notation. |
| Check / invoice amount converter | Convert `25.40` to a legal/check line with a fractional cents form and optional `only`. | Yes | `check` style returns `twenty-five and 40/100 dollars`; `only_suffix` appends `only`; currency setting controls unit words. |
| Currency amount converter | Spell the major and minor units, with common ISO currencies and zero-decimal currencies. | Yes | `currency` style supports 25 codes with unit/sub-unit words; JPY/KRW round to whole units. |
| Ordinal converter | Support `first`, `twenty-first`, and digit suffix forms such as `21st`. | Yes | `ordinal` and `ordinal_num` styles cover both forms. |
| Year wording | Read four-digit years as people say them (`nineteen eighty-four`, `nineteen oh five`). | Yes | `year` style handles the common four-digit cases and falls back to cardinal wording otherwise. |
| Regional wording | Let users choose American no-`and` wording or British/Commonwealth `and` wording. | Yes | `use_and` checkbox toggles the final-group `and`. |
| Large-number naming | Choose short, long, or Indian lakh/crore scales. | Yes | `scale` enum supports `short`, `long`, and `indian` with documented caps. |
| Presentation controls | Upper/title/sentence case and hyphenated compounds. | Yes | `letter_case` and `hyphenate` options expose these controls. |
| Bulk conversion | Convert several numbers at once. | Yes | Input accepts up to 1,000 non-blank lines and returns one result per line. |
| Localization into many languages | Convert to languages other than English. | Out of model for this tool | Not built; the current block is English-only to avoid shallow or incorrect language rules. |
| Legal jurisdiction templates | Bank- or jurisdiction-specific legal wording and compliance advice. | Out of model | Not built; page copy tells users to follow their bank/template. |

## UX decisions

- Use a textarea so spreadsheet columns can be pasted directly.
- Use enum/select controls for style, scale, case, currency, and decimal handling; labels show worked examples where a plain value would be ambiguous.
- Use checkboxes for the two non-default toggles (`use_and`, `only_suffix`) and for default-on `hyphenate`.
- Add preset chips for common tasks: default words, cheque amount, Indian currency, spoken year, ordinal digits, and British `and` wording.
- Keep output as plain text for copy/paste into contracts, invoices, checks, forms, and documents.

## Verification targets from the scan

The final matrix should cover default cardinal output, a check amount with `only`, Indian currency wording, ordinal digit suffixes, British `and`, a non-default case value, a non-default decimal mode, and error handling for invalid input or unsupported limits.
