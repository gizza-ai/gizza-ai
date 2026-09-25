## About this tool

Elo updates are small enough to do by hand, but it is easy to mix up the expected score, the actual score and the K-factor. This calculator shows the whole two-player update: expected score for both sides, each signed rating change, the new ratings, and the formula with your numbers substituted into it.

Use `score_a=1` when Player A wins, `0.5` for a draw and `0` for a loss. For a same-opponent series, set **Games** to the number of games and enter Player A's total points as `score_a` — for example, `games=5` and `score_a=3.5`. Player B's score is implied as `games - score_a`.

### Worked example

A 1600-rated player beats a 1500-rated player with `K=32`:

```text
Player A: 1600 -> 1612 (+12)
Player B: 1500 -> 1488 (-12)

Expected: Player A 0.640065 (64.01%), Player B 0.359935 (35.99%)
Formula: E = 1 / (1 + 10^((opponent - player) / 400)), dR = K x (S - E)
```

The scenario block also shows what the same pairing would do for a draw or loss, so you can compare every outcome before a match.

### K-factors and caps

Common K-factors include 10 for very stable/master ratings, 20 for established ratings, 32 for many online-style examples and 40 for new or volatile players. The tool does not guess your federation or platform tier because real rules depend on history the calculator cannot see; pick the value that matches your system.

Set **Rating-difference cap** to `400` if you want the common FIDE-style rule where a huge mismatch is treated as at most 400 points apart when computing expected score. Leave it at `0` for the uncapped classical formula.

### Output formats

`summary` is the readable report. `scenarios` prints only the win/draw/loss table. `json` and `csv` are for scripts and spreadsheets, and `delta` returns just Player A's signed rating change. Use **Decimals** when you want fractional point changes instead of whole rating points.

## FAQ

<details>
<summary>What formula does this use?</summary>

The classical Elo expectation is `E = 1 / (1 + 10^((opponent - player) / 400))`. The rating change is `K × (S − E)`, where `S` is the actual score: 1 for a win, 0.5 for a draw and 0 for a loss. For several games against the same opponent, expected score and actual score are totals across the series.

</details>

<details>
<summary>What K-factor should I choose?</summary>

Use the value from the rating pool you are modelling. Chess examples often use 10 for very established players, 20 for standard established ratings, 32 for online examples and 40 for new players. The calculator leaves the choice explicit because FIDE, national federations and online sites use different rules.

</details>

<details>
<summary>Does this match Chess.com or Lichess ratings exactly?</summary>

No. Many online platforms use Glicko-family systems that also track uncertainty and inactivity, so the displayed rating can move differently from a plain Elo update. This tool is for classical Elo arithmetic and transparent what-if calculations.

</details>

<details>
<summary>Can I calculate a tournament with many opponents?</summary>

Run the tool once per opponent and add the deltas, or use the `games` field only when every game was against the same opponent. A true event table needs one opponent rating and score per row, which is a different input shape.

</details>

<details>
<summary>Why can Player B have a different K-factor?</summary>

Some systems assign different development factors to players depending on experience or rating band. If Player B's K-factor differs, set **K-factor for B**; if it is `0`, the tool mirrors Player A's K-factor. Different K-factors make the exchange non-zero-sum by design.

</details>
