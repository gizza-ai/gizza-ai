# Competitor analysis: caesar-cipher-breaker

Date: 2026-09-24
Tool: `caesar-cipher-breaker`

## Comparator scan

Reviewed common Caesar/ROT cracking tools and classical-cipher utility pages. Their overlapping expectations are small but clear:

| Competitor shape | Table-stakes behavior | In model? | Implemented decision |
| --- | --- | --- | --- |
| Caesar brute-force table | Show all 26 possible decryptions so users can choose by eye. | Yes | `output=all` prints every shift in order. |
| Automatic Caesar solver | Score shifts and put the likely plaintext first. | Yes | Default `best` view ranks shifts by language letter-frequency likelihood. |
| Ranked candidate UI | Show the next-best alternatives for short/noisy text. | Yes | `ranked` view lists 1-26 candidates with chi-squared, confidence, and preview. |
| Frequency-analysis explanation | Report confidence/score and warn on short text. | Yes | Output includes confidence, scored-letter count, short-text warning, and `report` diagnostics. |
| ROT13 support | Common Caesar special case. | Yes | ROT13 is naturally handled as shift 13; example chip included. |
| Preserve punctuation/case | Do not destroy spaces, punctuation, or capitalization. | Yes | Core preserves case and all non-letter characters. |
| Language profiles | Some solvers let users pick language. | Yes | English, French, German, Spanish, Italian, Portuguese profiles. |
| Digit shifting | Puzzle variants sometimes rotate digits too. | Yes | Optional `shift_digits` checkbox. |
| Break arbitrary substitution/Vigenere | Non-Caesar cracking. | Out of model | Not built; report copy explains the Caesar-only scope. |

## UX decisions

- Default to the best plaintext because most users want the answer quickly.
- Add ranked/all/report views for short messages, classroom demos, and debugging.
- Use a slider for the ranked `top` count and enum selects for output/language.
- Keep plaintext as copyable text; no binary/media output.

## Verification targets

Cover default best output, ranked deep-link with top=3, all-shifts count, digit rotation enabled, CLI exact output, invalid language/output errors, and the page's short-input warning path.
