## About this tool

Use the tournament bracket generator when you need a printable knockout sheet from a plain roster. Paste one team or player per line, choose single or double elimination, and the tool creates the first-round pairings, later-round placeholders, automatic byes, and optional consolation matches. The entered order can be treated as seed order, used exactly as typed, or shuffled with a reproducible draw seed.

### Worked example

For a four-team single-elimination bracket with a third-place match:

```bash
gizza tool tournament-bracket-generator participants=$'Lions\nTigers\nBears\nSharks' output_format=text third_place_match=true tournament_name='Spring Cup'
```

The bracket begins:

```text
Spring Cup
Single elimination · 4 participants · 4-slot bracket · 0 byes · 2 rounds · 4 matches
Seeds: 1 Lions · 2 Tigers · 3 Bears · 4 Sharks

Round 1 — Semifinals
  M1  Lions (1) vs Sharks (4)
  M2  Tigers (2) vs Bears (3)
```

### Limits and edge cases

- Accepts 2 to 64 participants.
- A bare number such as `16` expands to `Team 1` through `Team 16`.
- Blank lines, `#` comment lines, and simple list markers (`-`, `*`, `1.`) are ignored.
- Names must be unique case-insensitively.
- Non-power-of-two fields are rounded up and spare slots are printed as `BYE`; standard seeding gives those byes to the highest seeds.
- Double elimination includes the winners bracket, losers bracket, grand final, and optional reset match; round-robin pools and live score advancement are outside this local generator.

## FAQ

<details>
<summary>How are byes assigned?</summary>

The bracket size is rounded up to the next power of two. With standard seeding, the missing low-priority slots become `BYE`, so the highest seeds receive the byes and do not meet each other early.

</details>

<details>
<summary>What is the difference between standard, ordered, and random seeding?</summary>

`standard` treats the roster order as seed order and places seed 1 against the last slot, seed 2 on the opposite half, and so on. `ordered` fills slots top to bottom so entry 1 plays entry 2. `random` shuffles the roster first, using the numeric draw seed so the same inputs reproduce the same draw.

</details>

<details>
<summary>Can I export the bracket?</summary>

Yes. Choose text for a printable sheet, markdown for a table, CSV for a spreadsheet, or JSON for automation. The page output can be copied, and the CLI returns the same formats.

</details>

<details>
<summary>Does this update winners as matches are played?</summary>

No. It generates the bracket structure before play starts. Later rounds use placeholders such as `Winner of M1` and `Loser of M2`; tracking live results or publishing a hosted bracket is outside this local pure-Rust tool.

</details>
