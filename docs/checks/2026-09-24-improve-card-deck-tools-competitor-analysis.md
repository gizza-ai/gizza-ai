# card-deck-tools — competitor analysis (2026-09-24)

Scan run while the tool was being finished, so the descriptor shipped with the table-stakes already
in it. All notes are **paraphrased** from search-result summaries of each tool's own description; no
competitor copy, branding, trademarks, or markup was reproduced. Profiles are **index-level** — the
competitor pages were not fetched and rendered, so each feature list is treated as *claimed* rather
than verified. Where a capability was only claimed by one tool it is marked as such below.

## Competitors reviewed

| # | tool / source | what it is |
| - | ------------- | ---------- |
| 1 | RANDOM.ORG — Playing Card Shuffler (`random.org/playing-cards/`) | the best-known shuffler; draws hands from decks shuffled with atmospheric-noise randomness |
| 2 | CalculatorSoup — Random Card Generator (`calculatorsoup.com`) | a draw-focused generator over a *configurable* deck (ranks, suits, jokers) |
| 3 | EasyProTools — Shuffle a Deck of Cards (`easyprotools.com`) | a shuffle-then-deal tool that names Fisher–Yates and offers several shuffle methods |

### 1. RANDOM.ORG — Playing Card Shuffler
- Core model: shuffle one or more decks, then deal a number of hands with a chosen number of cards
  per hand; the remainder stays in the deck.
- Selling point is the randomness *source*: atmospheric noise rather than a software PRNG, pitched
  explicitly as better than "the pseudo-random number algorithms typically used in computer
  programs".
- Deck composition (deck count, joker inclusion) is configured on the page; cards are shown as
  images rather than text.

### 2. CalculatorSoup — Random Card Generator
- Draws N cards per play from a standard 52-card deck **or** a custom deck the user composes by
  ticking which number cards, face cards, suits and jokers are included. Deck size is
  `ranks × suits + jokers`.
- Reshuffles before each draw.
- Has an explicit **reuse vs. remove** switch: reused cards return to the deck before the next draw
  (draw with replacement), removed cards do not (draw without replacement).
- Presentation control: how many cards to show per displayed row.

### 3. EasyProTools — Shuffle a Deck of Cards
- Fisher–Yates is named as the primary shuffle, alongside a claimed six additional shuffle methods
  (riffle/overhand-style simulations).
- After shuffling, it distributes cards to an arbitrary number of players with an arbitrary number
  of cards per hand — the dealing model this tool matches.
- Positions itself as "mathematically rigorous" and fully in-browser.

## Table-stakes → where each landed

| table-stake (seen at ≥1 competitor) | verdict | how |
| ----------------------------------- | ------- | --- |
| Shuffle a standard 52-card deck | **in-model** | `mode=shuffle` lists the full post-shuffle order |
| Unbiased Fisher–Yates shuffle | **in-model** | `shuffle()` walks top-down with rejection-sampled indices (no modulo bias); a core test asserts the top card varies across 400 seeds, which the classic off-by-one bug would fail |
| Deal N players × M cards | **in-model** | `mode=deal` with `players` (1–52) and `cards_per_player` (1–52), dealt round-robin exactly as a dealer does it |
| Draw N cards off the top | **in-model** | `mode=draw` with `count` |
| Draw *with replacement* (reuse) vs. *without* (remove) | **in-model** | `replacement` checkbox; with it on, `count` may exceed the deck size and repeats are expected |
| Multiple decks stacked / shoe play | **in-model** | `decks` 1–8 (6 and 8 are the usual casino shoe sizes) |
| Jokers included or excluded | **in-model** | `jokers` 0–2 per deck; jokers render `JK` and are never given a poker ranking |
| "Cards remaining in the deck" readout | **in-model** | every mode ends with a `Remaining in deck:` line; a replacement draw says so explicitly |
| Sorting a hand for readability | **in-model** | `sort_hands` orders each hand high to low by rank, then S/H/D/C |
| Control over how cards are written | **in-model** | `notation` enum: `short` (AS), `symbol` (A♠), `long` (Ace of Spades) — `long` also switches the layout to one card per line |
| Copy / download / reset the result | **in-model, already platform** | the page generator gives every `format = "text"` tool Copy + Download + Reset |
| Preset game setups (poker / hold'em / bridge) | **in-model** | `[[example]]` chips prefill and run each setup in one click |

### Beyond every competitor (our differentiators)
- **A seed.** None of the three exposes one. `seed` makes the whole result reproducible: the same
  seed and settings always rebuild the same deck, so a deal can be cited, shared as a link, tested
  against, or replayed. A plain whole number is used directly; any other text (`table-3`) is hashed,
  so seeds can be human-readable.
- **Poker ranking of the dealt hands.** `evaluate` labels every 5–7 card hand with its best
  five-card ranking ("Royal flush", "Full house, 7s over Queens", "Pair of 7s"), choosing the best
  of all 21 five-card subsets for a seven-card hand. The competitors deal cards but do not score them.
- **Deep-linkable state.** Every parameter is a query param, so a specific deal is a URL.
- **Reachable from chat and the CLI**, not only a web form.

## Out-of-model (considered, NOT built)
- **True randomness from atmospheric noise** (competitor 1's headline feature) — needs a network
  fetch to a third-party entropy service. This tool is local-only and deterministic by design; the
  page and descriptor both state plainly that it is built for reproducibility, *not* secrecy, and is
  not cryptographically secure.
- **Card face images / a visual table layout** — the page model here is one deterministic text
  output that must round-trip through chat and the CLI unchanged. `notation=symbol` is the
  text-native compromise.
- **Simulated physical shuffle methods** (riffle, overhand, pile) — they are *worse* shuffles than
  Fisher–Yates, and offering them implies a fidelity claim about hand-shuffle bias that this tool
  cannot honestly verify.
- **Custom deck composition by individual rank/suit** (competitor 2) — a per-rank/per-suit tick grid
  does not fit the flat key=value parameter model shared by chat, CLI and page, and every named card
  game this tool targets uses the full deck.
- **Non-standard decks** (tarot, Uno-style, regional 32/40-card packs) — a genuinely different deck
  model; would be its own tool rather than a mode here.
- **Persisting a shuffled deck across draws** (a stateful shoe you keep drawing from) — blocks are
  pure and stateless. The seed covers the same need: the same seed reproduces the same deck, and
  `mode=shuffle` shows the whole order at once.
- **Account-saved tables or multiplayer dealing** — no accounts, no server.

## UX control patterns matched
- Mode switch (shuffle / deal / draw) → a real `<select>` from `Param::enumv`, deep-linkable as
  `?mode=draw`.
- Game presets → `[[example]]` chips (poker, hold'em, bridge, single draw, six-deck shoe).
- Reuse/remove switch → the `replacement` checkbox, deep-linkable as `?replacement=true`.
- Cards-per-row presentation control → folded into `notation` (13 per line for the compact
  notations, one per line for `long`), so there is no separate knob to get wrong.
