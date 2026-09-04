# Substitution solver competitor analysis (2026-09-04)

Tool: `substitution-solver` — solves a monoalphabetic substitution cipher via deterministic hill-climbing plus manual decode/encode/analyze helpers.

Search query used: `monoalphabetic substitution cipher solver online hill climbing cribs frequency analysis`.

## Competitors skimmed

1. Cipher Decipher hill-climbing cipher solver
   - Table stakes: paste ciphertext, automatic monoalphabetic solve, clear statement that polyalphabetic/homophonic/polygraphic ciphers are out of scope, English-statistics scoring.
   - UX patterns: single large text area, solve action, explanatory limits.
   - In-model: automatic solve, input text area, out-of-scope notes.
   - Out-of-model: none needed for local browser execution.

2. Text Machine substitution cipher solver
   - Table stakes: crack cryptograms with frequency/trigram analysis, no-key automatic recovery, examples that show ciphertext to plaintext.
   - UX patterns: paste text, immediate/result panel, SEO copy around cryptograms.
   - In-model: deterministic automatic solve, worked example, browser-local page.
   - Out-of-model: any server-side batch/session features are not relevant here.

3. TryDevTools substitution cipher solver
   - Table stakes: frequency-analysis aid, manual refinement of mappings, decode preview.
   - UX patterns: ciphertext text area, visible mapping/key controls, frequency hints.
   - In-model: manual `key` decode/encode, `analyze` mode with frequency table, crib field for locked mappings.
   - Out-of-model: fully interactive drag/drop alphabet mapping grid would require custom per-tool UI; represented with a 26-letter key field instead.

4. MatrixPuzzle substitution solver
   - Table stakes: manual map, live decryption preview, frequency analysis hints.
   - UX patterns: worksheet-style cryptanalysis controls.
   - In-model: decode mode, frequency report, cribs.
   - Out-of-model: rich mapping workbench is listed but not built; the generic gizza form model favors reusable text/select/checkbox fields.

## Decisions for this build

- Parameters shipped: `text`, `mode` (`solve`, `decode`, `encode`, `analyze`), optional 26-letter `key`, optional `cribs`, `effort` (`quick`, `standard`, `thorough`), and `keep_layout`.
- Defaults: mode `solve`, effort `standard`, preserve layout `true`; blank key/cribs for automatic solve/analyze.
- Examples: Atbash decode, frequency analysis, and solve-with-cribs preset chips.
- Output: text report with plaintext/ciphertext, key block, confidence/fitness, counts, and notes.
- Limits documented: English monoalphabetic only, heuristic solve, 100,000-character input cap, first 1,200 letters searched, ASCII A-Z substitution only.
- Rejected/out-of-model for now: custom interactive alphabet-grid editor and cloud/server batch solving. Both can be useful but are not required for a generic, local, reusable gizza tool page.
