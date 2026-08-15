## About this tool

A **trie** (prefix tree) is the data structure behind almost every search box that suggests as you
type. Each node is one character, words that begin the same way share the same path, and asking
"what starts with `app`?" is a short walk down that path followed by a sweep of everything below it.

This tool builds that trie from a wordlist you paste and answers the prefix query the way a real
typeahead would: ranked, capped to a handful of suggestions, and reporting how many terms matched in
total. It also shows the structure it built — node count, depth, and how many characters the shared
prefixes saved — so you can sanity-check a suggestion dictionary before shipping it. Everything runs
locally in your browser; the wordlist is not uploaded.

### Worked example

Wordlist:

```
apple,12
application,7
apply,3
apricot,5
banana,9
band,4
```

Typed prefix `app`, ranked by weight:

```
Suggestions for "app": 3 of 3 matches
Trie: 6 terms, 26 nodes, depth 11, 25 of 38 characters stored (34.2% saved)

1. apple        weight 12
2. application  weight 7
3. apply        weight 3
```

The same wordlist with the prefix `aple` and **typo tolerance** set to 1 still finds `apple`, marked
so you can tell an exact hit from a rescued one:

```
1. apple  weight 12  (1 typo)
```

### Wordlist format

One term per line. A term may carry a weight, either after the term or before it, separated by a
tab, a pipe, or a comma — all of these are read as `apple` with weight `12`:

```
apple,12
apple|12
12	apple
```

A line with no weight counts as `1`, and a term repeated on several lines has its weights **added**.
That means a plain pasted log of what people typed works directly as a frequency-ranked dictionary,
with no counting step first. Lines starting with `#` are ignored, so you can comment a list.

### What you can control

- **Typed prefix** — the characters someone has typed. Leave it empty to see the highest-ranked
  terms in the whole list, which is what a search box shows before the first keystroke.
- **Max suggestions** — 1 to 100. The total number of matching terms is reported regardless, so a
  cap of 5 still tells you there were 240 matches.
- **Rank by** — weight (highest first, ties alphabetical), plain alphabetical, or shortest term
  first.
- **Typo tolerance** — 0, 1, or 2 edits. Above 0, terms whose own prefix is within that many
  insertions, deletions, or substitutions of what was typed also match. Exact-prefix matches always
  rank above rescued ones, and each rescued suggestion is labelled with its edit count.
- **Case-sensitive** — off by default, so `Apple` and `apple` are one term and the first spelling
  seen is displayed. On, they stay separate terms and the prefix must match case too.
- **Output format** — the ranked list, JSON for scripting, or a drawing of the trie branch under
  your prefix with `*` marking every node where a stored term ends.

### Limits and edge cases

- Up to **20,000 terms**, each at most **200 characters**. Longer input is rejected with the
  offending line number rather than silently truncated.
- The trie drawing stops after **2,000 nodes** and says so; use a longer prefix to narrow it.
- Ranking is fully deterministic: equal weights break alphabetically, so the same input always
  produces the same output.
- Weights may be any finite number, including decimals and negatives. A line whose trailing field
  is not a number is treated as part of the term, so `new york, ny` stays one term.
- Matching is by Unicode character, not by byte, so accented and non-Latin terms work. Case folding
  uses Unicode lowercasing when case-sensitive matching is off.
- This is a dictionary autocompleter over terms you supply. It does not search inside documents —
  for ranked document or snippet search, use a full-text search tool instead.

## FAQ

<details>
<summary>How do I give terms a popularity ranking?</summary>

Two ways, and you can mix them. Add an explicit weight to each line (`checkout,120`), or simply
repeat a term once per occurrence and let the tool count them — repeated terms sum their weights, so
a raw list of past queries becomes a frequency-ranked dictionary with no preprocessing. Weighted and
unweighted lines can appear in the same list; an unweighted line contributes `1`.

</details>

<details>
<summary>What does the typo tolerance actually match?</summary>

It compares what you typed against each term's own leading characters, allowing up to the chosen
number of edits (insertions, deletions, or substitutions). With a budget of 1, `aple` reaches
`apple` because one insertion turns the typed text into that term's prefix. Terms that match the
prefix exactly are always listed first, and every rescued suggestion shows how many edits it needed,
so a wrong-but-close result is never disguised as a clean hit.

</details>

<details>
<summary>Why does the output mention nodes and depth?</summary>

They describe the tree that was built. `nodes` is how many characters are actually stored once
shared prefixes are merged, `depth` is the longest term's length, and the "characters stored" figure
compares the two against the raw character count of your list. A high saving means your dictionary
has a lot of shared prefixes and will compress well; a saving near zero means the terms diverge
immediately and a trie buys you little over a sorted list.

</details>

<details>
<summary>Can I see the tree itself?</summary>

Yes — set the output format to **Trie drawing**. It prints the branch under your typed prefix, one
character per line, indented by depth, with `*` on every node where a stored term ends and the full
term and weight beside it. That makes shared prefixes and terminal nodes visible directly. The
drawing follows the exact prefix only, so typo tolerance does not widen it.

</details>

<details>
<summary>What happens if nothing matches the prefix?</summary>

The result says so explicitly instead of returning an empty list, and still reports the trie
statistics so you know the wordlist was parsed. When typo tolerance is off, it also suggests raising
it, which is usually the difference between "this word is not in the list" and "this word is in the
list, spelled slightly differently".

</details>

<details>
<summary>Is my wordlist uploaded anywhere?</summary>

No. The trie is built and queried by WebAssembly running inside your own browser tab, so the pasted
terms never leave the page. The same code is available from the command line if you would rather
pipe a dictionary in from a script.

</details>
