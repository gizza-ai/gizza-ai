## Turn a list of names into a fair fixture list

A **round robin** is the format where everybody plays everybody. It is the fairest way to rank a league, a club night or a pool stage, and it is also the most tedious to write out by hand — six teams already means fifteen matches spread over five rounds, and the moment you have an odd number of entrants someone has to sit out each round without sitting out twice.

This tool does the bookkeeping. Paste your teams or players — one per line, or a single comma-separated line, or just the number of entrants — and it lays out every match, grouped into rounds, using the **circle method**: one participant is held still while the rest rotate one seat per round, which guarantees that after `n − 1` rounds every pair has met exactly once and no participant is double-booked inside a round.

Three things it gets right that a hand-written grid usually doesn't:

- **Byes rotate.** With an odd count a phantom entrant joins the draw, so each round has exactly one participant resting and — across the whole schedule — everybody rests exactly once.
- **Home and away stay balanced.** Fixtures are oriented so no participant hosts more than one game above anyone else. A **double round robin** appends a mirrored return leg with the venue swapped, which makes the split exact.
- **Courts are assigned.** Give it a count (`4`) or your own venue names (`North Field, South Field`) and each round's simultaneous matches are dealt across them.

Everything runs in your browser. The roster never leaves the page, and the same input always produces byte-identical output — so you can regenerate a schedule mid-season and get the same fixtures back. Set a non-zero **draw seed** if you want the running order shuffled instead, reproducibly.

### Worked example

Six teams, entered one per line:

```text
Lions
Tigers
Bears
Wolves
Hawks
Sharks
```

Six entrants make 6 × 5 ÷ 2 = **15** matches over **5** rounds, three per round:

```text
Format: single round robin
Participants: 6
Rounds: 5
Matches: 15
Matches per participant: 5

Round 1
  1. Lions vs Sharks
  2. Tigers vs Hawks
  3. Bears vs Wolves

Round 2
  1. Hawks vs Lions
  2. Sharks vs Wolves
  3. Tigers vs Bears

Round 3
  1. Wolves vs Lions
  2. Hawks vs Bears
  3. Sharks vs Tigers

Round 4
  1. Lions vs Bears
  2. Wolves vs Tigers
  3. Hawks vs Sharks

Round 5
  1. Lions vs Tigers
  2. Bears vs Sharks
  3. Wolves vs Hawks
```

Every pair appears once — Lions meet Sharks, Hawks, Wolves, Bears and Tigers across the five rounds — and each team is listed first (at home) either two or three times.

Switch **Output format** to CSV for a spreadsheet, Markdown for a wiki or pull request, or JSON to drive a script. The CSV and JSON views carry the columns `round`, `match`, `home`, `away` and, when you set courts, `court`.

### FAQ

<details>
<summary>How many rounds and matches will I get?</summary>

With `n` participants there are always `n × (n − 1) ÷ 2` matches in a single round robin. An **even** `n` plays them over `n − 1` rounds with `n ÷ 2` matches per round. An **odd** `n` needs `n` rounds of `(n − 1) ÷ 2` matches, because one participant rests each round. A double round robin doubles both figures.

</details>

<details>
<summary>What happens when I have an odd number of teams?</summary>

A phantom entrant joins the draw, and whoever is paired with it that round gets a **bye** — shown as a `Bye:` line in the text output and as an `away` value of `BYE` in the table formats. The phantom rotates like everyone else, so each participant rests exactly once (twice in a double round robin). Turn off **Show byes** if you only want the playable fixtures.

</details>

<details>
<summary>Is home and away actually balanced?</summary>

Yes. Fixtures are oriented so that the difference between the most and least frequent host is never more than one — which is the best possible when each participant plays an odd number of games. Choosing **double round robin** makes it exact: the second leg is the first leg with home and away swapped, so every pair meets once at each venue.

</details>

<details>
<summary>How do I enter my participants?</summary>

One name per line is the normal shape. A single comma-separated line (`Alice, Bob, Carol`) works too. Leading list markers — `-`, `*`, `1.`, `2)` — are stripped, blank lines are skipped, and any line starting with `#` is treated as a comment, so you can paste straight from a notes app. If you only type a number, such as `8`, you get eight placeholder entrants named `Team 1` … `Team 8`.

</details>

<details>
<summary>Can I assign courts, fields or time slots?</summary>

Courts and fields, yes: enter a count like `4` to label them `Court 1` … `Court 4`, or type your own names such as `North Field, South Field`. Each round's simultaneous matches are dealt across them in order. Clock times are not generated — the rounds are ordered but not timed, so add start times in your spreadsheet after exporting to CSV.

</details>

<details>
<summary>Will I get the same schedule if I run it again?</summary>

Yes — the generator is fully deterministic, so identical input gives byte-identical output and you can safely regenerate a fixture list you have already published. If you would rather the draw were mixed up, set **Draw seed** to any non-zero number: that shuffles the entry order reproducibly, so the same seed always yields the same schedule.

</details>

<details>
<summary>Can I continue an existing schedule's round numbering?</summary>

Set **First round number**. The schedule is numbered from there upward, so a second half-season generated with a first round of `6` continues cleanly from a first half that ended at round 5.

</details>

### Limits and edge cases

- **2 to 64 participants.** Fewer than two has nothing to schedule; the upper bound keeps the fixture list printable (64 entrants already means 2,016 matches over 63 rounds).
- **Names must be unique**, compared without regard to case — `Lions` and `lions` are rejected as a duplicate rather than silently merged.
- **Up to 32 courts or venues.** Courts label parallel slots within a round; the tool does not check that you actually have that many free at once.
- **No times, no constraints.** Rounds are ordered but not scheduled to a clock, and there is no way to say "these two can't play in round 3" or "this team is away in week 1".
- **No standings.** This generates the fixture list only — scores, points tables and tie-breaks are out of scope.
- **The summary block applies to the text and Markdown formats.** CSV and JSON always contain just the fixtures, so they stay directly parseable.
