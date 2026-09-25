# elo-rating-calculator — competitor analysis (2026-09-25)

Scan run before implementation (new tool built via `/create-next-tool`, so Phase 1 "verify the
existing tool" had no prior surface to verify — the competitor scan drove the initial descriptor
instead of a post-hoc gap close). Five reachable competitor tools were profiled. Everything below is
**paraphrased**; no competitor copy, branding, or trademarks were reproduced or reused.

Two initially-picked competitors were unreachable and replaced per the skill's rule
(`elocalculator.com` — expired TLS certificate; `teamupgg.com/elo-calculator` and
`yottachess.com/eloCalculator` — HTTP 403 to the fetcher). Replacements: `chess.lc` and
`calculator.academy`.

## Competitor profiles

### 1. Omni Calculator — Elo Calculator
- **url:** https://www.omnicalculator.com/sports/elo
- **features:** single-game rating change; a mode for combining several results against different
  opponents; expected win probability; new rating.
- **params_options:** player rating (number), opponent rating (number), game selector (starts at
  "Game 1"), result as 1 / 0.5 / 0, opt-in checkbox to override the K-factor by hand.
- **input/output formats:** numeric fields in, numeric read-outs out.
- **output_quality:** shows expected winning probability, the rating delta, and the post-game rating.
- **ux_patterns:** live recompute as you type; "reload"/"clear all" reset affordances; multi-game rows
  appear progressively; explanatory FAQ under the widget.
- **limits:** no free-for-all or team support; framed around chess and comparable systems.
- **free_vs_paid:** free, no account.

### 2. Chessigma — Chess Elo Rating Calculator
- **url:** https://www.chessigma.com/tools/elo-calculator
- **features:** win-probability bar for the matchup; **all three outcomes priced at once** (the Win /
  Draw / Loss buttons each carry their own point delta); new rating per outcome.
- **params_options:** your rating and opponent rating as drag/scroll-adjustable numbers; K-factor
  presented as the FIDE tier set 10 / 20 / 40.
- **output_quality:** expected-score percentage, likely winner, per-outcome deltas, resulting rating.
- **ux_patterns:** the matchup odds are rendered as a two-sided percentage split; outcome buttons
  double as the result picker and the what-if table; cross-platform framing (FIDE / Chess.com /
  Lichess) with a note that the scales are offset by a couple hundred points.
- **limits:** states ratings are not directly comparable across platforms, and that federation
  ratings come only from over-the-board rated play.
- **free_vs_paid:** free, no signup.

### 3. Coddy — Elo Calculator (step by step)
- **url:** https://coddy.tech/chess/tools/elo-calculator
- **features:** **formula substitution written out with the user's own numbers**; automatic K-factor
  from experience level with a manual override; one row per opponent for a whole event; a
  performance-rating read-out after several games.
- **params_options:** current rating, number of rated games played, then per-row opponent rating +
  result; K-factor field.
- **output_quality:** new rating, point change, expected score, actual score, and the K used; all
  three single-game outcomes shown side by side.
- **ux_patterns:** teaching-first layout — every intermediate value is displayed rather than just the
  answer.
- **limits:** applies a **400-point cap on the rating difference** used for the expectancy; FAQ
  flags that online sites use Glicko-family systems, that ratings are not portable, and that
  performance rating degenerates at a perfect or zero score.
- **free_vs_paid:** free.

### 4. Chess.lc — ELO Calculator
- **url:** https://chess.lc/tools/elo-calculator
- **features:** single-game expectancy + delta + new rating; discloses both formulas on the page.
- **params_options:** your rating (pre-filled around 1200), opponent rating, result win/draw/loss,
  K-factor as three labelled presets (40 new / 20 standard / 10 master).
- **output_quality:** expected win probability as a percentage, signed delta, new rating.
- **ux_patterns:** labelled K presets instead of a bare number; formulas printed verbatim next to the
  result; no-signup / unlimited-use positioning.
- **limits:** single game only, no aggregation; explicitly says it does not model the
  uncertainty-tracking systems online platforms use.
- **free_vs_paid:** free.

### 5. Calculator Academy — Elo Rating Calculator
- **url:** https://calculator.academy/elo-rating-calculator/
- **features:** **reverse solving** — a "solve for" picker rearranges `new = old + K(S − E)` to return
  any one of the five quantities; worked calculation steps.
- **params_options:** old rating, K-factor, actual score, expected score (all four as plain numbers,
  example values 1500 / 32 / 1 / 0.5).
- **output_quality:** the solved value plus a breakdown of how the inputs combined.
- **ux_patterns:** Reset restores the example values; blank fields are not silently treated as zero;
  results are cleared when inputs change so a stale answer can't be misread.
- **limits:** states plainly that it implements only the algebra — no federation eligibility rules,
  rating floors, lookup tables, rounding conventions, or batch processing.
- **free_vs_paid:** free.

## Gap list vs our descriptor (fit-to-model tagging)

| # | Gap (≥1 competitor ships it) | Dimension | Tag | Disposition |
| - | ---------------------------- | --------- | --- | ----------- |
| 1 | Expected score for **both** sides, not just the player | capabilities | in-model | Built — both players' expectancy, percentage, delta and new rating are always reported. |
| 2 | New rating for the **opponent** too (zero-sum view) | capabilities | in-model | Built — `player_b` is a first-class side everywhere (summary, JSON, CSV). |
| 3 | All three outcomes priced at once (Chessigma, Coddy) | capabilities/UX | in-model | Built — a "Scenarios" block in `summary`, plus a dedicated `output_format = "scenarios"`. |
| 4 | Formula substitution with the user's values (Coddy, Chess.lc, Calculator Academy) | capabilities/copy | in-model | Built — `summary` prints `E = 1 / (1 + 10^((…) / 400))` and `dR = K * (S − E)` with the real numbers and the raw→rounded delta. |
| 5 | Labelled K-factor presets 10 / 20 / 40 (Chess.lc, Chessigma) | UX | in-model | Built — `[[example]]` preset chips for K = 10 / 20 / 32 / 40 and the tiers documented in the param description + FAQ. |
| 6 | 400-point rating-difference cap (Coddy) | capabilities | in-model | Built — `max_rating_difference` (0 = off, set 400 for the FIDE rule), applied symmetrically to the expectancy only. |
| 7 | Multi-game / whole-event aggregation (Omni, Coddy) | capabilities | in-model (partial) | Built for a **series against the same opponent**: `games` + a total `score_a`. A full per-opponent tournament table is listed below as considered-rejected. |
| 8 | Asymmetric K per player (FIDE allows it; implied by Coddy's per-player K) | capabilities | in-model | Built — `k_factor_b` (0 = mirror `k_factor`). |
| 9 | Rounding conventions (Calculator Academy calls out that it skips them) | capabilities | in-model | Built — `decimals` (default 0 = whole rating points, FIDE style); the delta is rounded once and then added, so `before + change = after` always holds. |
| 10 | Machine-readable / copyable output | capabilities | in-model | Built — `json`, `csv`, and a bare signed `delta` output format (no competitor offers these). |
| 11 | Named players rather than "You"/"Opponent" | copy/UX | in-model | Built — `player_a_name` / `player_b_name` flow into the summary, JSON and CSV. |
| 12 | Blank fields must not be read as zero (Calculator Academy) | UX | in-model | Built — the web wrapper treats an empty numeric field as the documented default, never 0. |
| 13 | Platform-comparability caveats + Glicko framing (Chessigma, Coddy, Chess.lc) | copy/SEO | in-model | Built — page copy and FAQ state that Chess.com/Lichess use Glicko-family systems and that scales are not interchangeable. |
| 14 | Reverse solve for K / expected / actual (Calculator Academy) | capabilities | considered, rejected | The tool already prints E and the raw delta, so the only genuinely missing direction is "what K produced this change", which is one division a user can do from the printed substitution. A `solve_for` mode would double the schema and make `score_a`/`k_factor` conditionally-required — schema bloat for a marginal case. |
| 15 | Automatic K-factor from experience (Omni, Coddy) | capabilities | considered, rejected | The real FIDE rule needs data this tool cannot see: number of rated games ever played, whether the rating has **ever** reached 2400, and under-18 status — and every federation/platform differs. Guessing it would be confidently wrong; the tiers are documented instead so the user picks. |
| 16 | Per-opponent tournament table (Omni multi-game, Coddy event rows) | capabilities | considered, rejected | A list-of-opponents input needs a repeating row control the generator does not have, and the aggregate is just the sum of independent pairings — running the tool once per opponent gives the identical total. Revisit if a declarative row-list control lands in the generator. |
| 17 | Performance rating (Rp) after an event (Coddy) | capabilities | considered, rejected | Meaningless for a single pairing (it diverges at a 100% or 0% score, which is the common case here) and it belongs to the rejected multi-opponent table. |
| 18 | Free-for-all / team Elo (Team Up) | capabilities | out-of-model for this tool | Needs an arbitrary list of participant ratings and a pairwise-averaging convention that is not part of classical Elo; a separate tool, not a param here. |
| 19 | Accounts, rating history, saved players | UX | out-of-model | Requires a backend/login; gizza tools are browser-local, no-account, no-server. |
| 20 | Live win-probability bar graphic (Chessigma) | visual | considered, rejected | The shared generator has no bar-chart primitive, and adding one for a single tool would be a per-tool hack; the percentages are printed instead. |

## Notes

- Everything shipped runs fully local (wasm) with no network, no account, and no server — the
  fit-to-model filter.
- No competitor asset, string, or layout was copied; the page copy, examples, FAQ and output layout
  are original.
