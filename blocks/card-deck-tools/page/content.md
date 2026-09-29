## About this tool

Shuffle a standard playing-card deck, deal hands, or draw cards with a reproducible seed. The same seed and settings always produce the same shuffled deck, which makes examples, tests, classroom demos, and tabletop setups easy to replay.

Use `mode=deal` for poker, bridge, classroom groups, or any round-robin deal. Use `mode=draw` when you just need the next N cards off the top, with optional replacement. Use `mode=shuffle` to list the full post-shuffle order.

### Worked example

Deal four sorted five-card hands and label their poker rankings:

```bash
gizza tool card-deck-tools 'mode=deal' 'players=4' 'cards_per_player=5' 'seed=42' 'sort_hands=true' 'evaluate=true'
```

The result includes one line per player and a `Remaining in deck:` count. Change only `seed` to get a different deal; reuse the same seed to reproduce the original handout exactly.

### Limits and edge cases

- `decks` is capped at 8 and `jokers` at 2 per deck, for a maximum deck size of 432 cards.
- A deal without replacement must fit in the deck: `players × cards_per_player` cannot exceed the deck size.
- A draw without replacement cannot ask for more cards than the deck contains. Turn on `replacement=true` when repeated cards are allowed.
- Poker evaluation applies only to 5–7 card hands without jokers. Other hands are still dealt, but their ranking note says why they are not ranked.
- The shuffle is deterministic and unbiased for reproducible tooling. It is not cryptographically secure and should not be used for gambling or secrets.

## FAQ

<details>
<summary>Why use a seed instead of true randomness?</summary>

A seed makes the result reproducible. If you send someone the same parameters, they can rebuild the exact same shuffle, deal, or draw. That is useful for tests and examples; it is not intended to replace secure randomness.

</details>

<details>
<summary>How are cards dealt to players?</summary>

Deals are round-robin, like a human dealer: one card to player 1, one to player 2, and so on until every player has the requested number of cards. The remaining count is reported after the hands.

</details>

<details>
<summary>Can I draw more cards than the deck has?</summary>

Only with `replacement=true`. Without replacement, each card is removed after it is drawn, so the tool rejects requests larger than the deck. With replacement, every draw starts from the full deck and repeated cards are expected.

</details>

<details>
<summary>What poker hands can be evaluated?</summary>

The `evaluate` option labels the best five-card ranking for hands of 5, 6, or 7 cards with no jokers. It reports hands with jokers or other sizes as not ranked instead of guessing house rules.

</details>
