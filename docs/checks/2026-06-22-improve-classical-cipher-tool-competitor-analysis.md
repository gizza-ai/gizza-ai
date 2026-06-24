# classical-cipher-tool — competitor analysis (2026-06-22)

Tool: encrypt/decrypt with the classic Caesar, Vigenère, Atbash, and rail-fence
ciphers, plus a Caesar brute-force that lists all 26 shifts. Pure-Rust, runs
entirely in the browser / chat sandbox / CLI — no network, no server.

## Surfaces verified

- **chat schema (drift guard):** `cargo test` — `schema_json_matches_authored_chat_schema` passes.
- **CLI:** `gizza tool classical-cipher-tool …` — caesar (encrypt + brute-force),
  vigenere, atbash, rail-fence all produce the expected ciphertext; bad cipher → exit 1
  with a clear error.
- **page:** 5 Playwright tests pass (caesar default, vigenère round-trip, atbash keyless,
  caesar brute-force, query-param deep-link prefill).

## Top competitors surveyed

1. **CyberChef (GCHQ)** — recipe-based "cyber swiss-army knife"; has ROT13, Vigenère,
   Atbash, rail fence, and a "ROT13 Brute Force" among hundreds of operations.
2. **dcode.fr** — large catalogue of individual classical-cipher pages (Caesar, Vigenère,
   Atbash, rail-fence / zig-zag), each with an automatic solver / brute-force.
3. **Boxentriq cipher tools** — Caesar/ROT, Vigenère, Atbash, rail-fence pages aimed at
   puzzle/escape-room solvers, with brute-force shift tables.
4. **cryptii.com** — pipeline-style encoder with Caesar, Vigenère, and other classical ciphers.
5. **rumkin.com / practicalcryptography.com** — long-standing classic-cipher calculators
   (Caesar, Vigenère, rail fence) plus reference explanations.

## Capability diff vs. competitors

| Capability | Competitors | This tool |
| --- | --- | --- |
| Caesar / ROT shift (any shift, mod 26) | yes | yes (negative + >26 wrap mod 26; ROT13 = shift 13) |
| Caesar brute-force (all 26 shifts) | CyberChef/dcode/Boxentriq | yes (`operation=brute-force`, `shift NN:` lines) |
| Vigenère encrypt/decrypt (keyword) | yes | yes (non-letter key chars ignored; keyless rejected) |
| Atbash (keyless, self-inverse) | yes | yes |
| Rail-fence / zig-zag (N rails) | yes | yes (2–64 rails, default 3; full-string transposition) |
| Case preserved, non-letters pass through | most | yes (substitution ciphers) |
| 100% local / private / offline | varies | yes (no network at all) |
| Available as chat tool + CLI + web page | no (web only) | yes (three surfaces) |

## Gaps considered and decisions

- **Vigenère automatic key-recovery / cryptanalysis (Kasiski, IoC)** — dcode/CyberChef
  offer an automated solver. This is a substantial cryptanalysis feature, not the core
  encrypt/decrypt scope, and is fuzzy (needs a dictionary/language model for scoring). Left
  out as out-of-scope for a deterministic pure tool; documented brute-force is provided only
  where it is exhaustive and unambiguous (Caesar's 26-key space).
- **More ciphers (Affine, Beaufort, Playfair, ADFGVX, substitution-with-alphabet,
  Polybius)** — each is a distinct algorithm; the backlog name scopes this tool to the four
  named ciphers + Caesar brute-force. Not added (scope creep / would be separate tools).
- **Preserve-vs-strip non-letters toggle** — competitors vary. Chose the conventional
  lossless behaviour (preserve case + pass non-letters through) so round-trips are exact;
  not exposing a toggle keeps the schema small and the behaviour predictable.
- **Auto-detect cipher** — too unreliable to be useful; omitted.

## Copy / UX / honesty

- Page, chat description, and content.md all state plainly that these are **educational /
  puzzle ciphers, NOT secure encryption** — matching how reputable competitors frame them and
  avoiding any false security claim.
- ROT13 is documented as Caesar shift 13 (a common search term) rather than added as a
  redundant option.
- No competitor copy, branding, or trademarks were copied; all wording is original.

## Out-of-model / not built

- Language-model-scored automatic Vigenère/substitution solvers (need a dictionary/ML scorer)
  — out of gizza's pure-deterministic model.
