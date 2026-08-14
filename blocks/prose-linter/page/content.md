## About this tool

Paste a draft to find common style problems before you publish or send it for review. The linter is deterministic and offline: it uses an embedded rule set, not an AI model, so the same input produces the same report every time.

It can flag tired clichés, vague weasel words, filler adverbs, hedges, corporate jargon, wordy phrases, redundancies, passive-voice patterns, repeated words, sentences that begin with "There is" or "So", long sentences, and opt-in E-prime violations. Use `checks` to run a subset such as `cliche,weasel,passive`, or start with `all,-passive` to disable one rule.

Example input:

```text
So the plan was written at the end of the day. We should leverage a best-in-class solution and touch base soon.
```

Example report output includes:

```text
5 issues found in 22 words / 2 sentences.

LINE:COL  RULE       ISSUE
1:1       so-start   sentence starts with "So"
1:18      passive    "was written" is passive voice
1:33      cliche     "at the end of the day" is a cliché
```

Limits and edge cases: input is capped at 1,000,000 bytes; matching is English-only; phrase rules are case-insensitive but not grammar-aware; passive voice is a heuristic that catches common "be + participle" forms and may miss complex constructions; spelling, punctuation, factual correctness, tone, and full grammar repair are out of scope. Treat findings as prompts for revision, not automatic edits.

## FAQ

<details>
<summary>Is this a grammar checker?</summary>

No. It does not parse full grammar, correct spelling, or rewrite sentences. It flags deterministic style patterns such as clichés, filler words, passive voice, repeated words, and long sentences so you can decide what to revise.

</details>

<details>
<summary>What should I put in the checks field?</summary>

Use `all` for the default rule set. You can name rules directly, such as `cliche,weasel,passive`, disable one default rule with a minus sign such as `all,-passive`, or add the stricter opt-in E-prime rule with `all,eprime`.

</details>

<details>
<summary>How does the ignore field work?</summary>

Enter a comma-separated list of words or phrases your style guide allows, such as `very, touch base`. Matching is case-insensitive and suppresses findings whose matched phrase exactly equals one of those entries.

</details>

<details>
<summary>Why did it flag a sentence that reads fine?</summary>

The tool is intentionally rule-based, so it can produce false positives. For example, a passive construction may be the clearest option, and a phrase that is cliché in marketing copy may be acceptable in dialogue. Review each finding in context.

</details>
