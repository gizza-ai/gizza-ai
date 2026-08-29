## About this tool

The CEFR level estimator gives a local, deterministic reading-difficulty estimate for English learner materials. It combines a small original banded vocabulary map, word-shape heuristics for words outside the map, a visible vocabulary coverage threshold, and sentence-complexity signals. The result is an A1-C2 headline with a decimal sublevel, a band profile, and the words that sit above your selected target learner level.

This is useful for teachers checking a worksheet, writers simplifying a passage, or curriculum teams comparing drafts. It is not a certification score and it does not use a licensed CEFR wordlist or a machine-learning model.

### Worked example

Input:

```text
Nevertheless, the methodology has significant implications for sustainability.
```

With target `B1`, output `annotated`, and unknown words set to `estimate`, the tool reports a high-level passage and marks words such as `methodology`, `implications`, and `sustainability` as above target. Switch to `table` for a spreadsheet-friendly word list or `json` for automation.

### Limits and edge cases

- English only. Non-English text may be tokenised, but the levels are not meaningful.
- The built-in lexicon is intentionally original and compact; unknown words are estimated from length, syllables, and academic-looking suffixes unless you choose another policy.
- Proper nouns are excluded by default so names such as cities or authors do not inflate the level. Turn on "Count proper nouns" if names are part of the learning burden.
- Maximum input is 200,000 characters. Very short texts are less stable because one hard word can dominate the profile.

## FAQ

<details>
<summary>Is this an official CEFR assessment?</summary>

No. It is a reproducible reading-difficulty heuristic for drafting and screening materials. Official CEFR placement needs validated tasks, human judgement, and learner performance data.

</details>

<details>
<summary>Why can I change the vocabulary coverage percentage?</summary>

Coverage controls the rule behind the vocabulary band. At the default 90%, the vocabulary level is the smallest CEFR band that covers at least 90% of recognised running words. Raising it makes occasional hard words matter more.

</details>

<details>
<summary>How are unknown words handled?</summary>

By default, unknown words are estimated from word length, syllable count, and academic suffixes. You can instead force all unknowns to C1 or C2 for a stricter pass, or exclude them from the profile when you only want recognised vocabulary counted.

</details>

<details>
<summary>Why are proper nouns ignored by default?</summary>

Names often look long or rare but do not always increase language difficulty. Excluding probable proper nouns keeps a passage about "Alex visited Edinburgh" from being rated harder just because of names.

</details>
