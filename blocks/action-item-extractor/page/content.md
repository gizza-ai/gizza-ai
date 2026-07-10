## Turn messy meeting notes into a checklist

Paste meeting notes, standup notes, or daily bullets and this tool extracts the lines that look like **action items** plus the lines that record **decisions**. It runs entirely in your browser and uses deterministic rules: explicit `ACTION:` / `task:` markers, `@handles`, owner assignments like `Alice will send the deck`, trailing owner tags, imperative task verbs, and decision words such as `Decided`, `agreed`, or `resolved`.

Because it is not an LLM, it does **not** invent unstated tasks or rewrite your meeting into a summary. The output is a structured task list you can copy into a project tracker.

Choose **Markdown** for a checklist, or **JSON** when you want to feed the result to a script. Keep the default **Group by type** to show one action checklist with owners inline, or choose **Group by owner** to make headings for each person. Turn off **Include decisions** when you only need tasks.

### Worked example

Input:

```text
ACTION: update the roadmap
Alice will send the deck
Book the venue @bob
Decided: ship on Friday
```

Output:

```markdown
## Action Items

- [ ] update the roadmap — _Unassigned_
- [ ] send the deck — **@Alice**
- [ ] Book the venue — **@Bob**

## Decisions

- ship on Friday
```

### FAQ

<details>
<summary>Does this use AI to infer tasks from vague prose?</summary>

No. It is intentionally deterministic: it only extracts lines with explicit action or decision signals. That keeps it predictable and avoids hallucinated tasks, but it also means subtle commitments hidden in prose may be skipped.

</details>

<details>
<summary>How are owners detected?</summary>

Owners come from `@handles`, leading assignments such as `Alice will send the deck` or `Bob to book the room`, owner markers such as `owner: Sam`, and short trailing tags such as `Prepare slides (Carol)`. Pronouns like `we` are not treated as owners.

</details>

<details>
<summary>What counts as a decision?</summary>

Lines with decision markers such as `Decided:`, `decision`, `agreed`, `resolved`, `approved`, `consensus`, or `final call` go into the Decisions section. Prefixes such as `Decided:` are stripped so the decision reads cleanly.

</details>

<details>
<summary>Why did a line not appear in the output?</summary>

It probably did not have an explicit signal. Add `ACTION:`, `task:`, an `@owner`, a leading owner assignment, or a listed action verb such as `send`, `review`, `schedule`, `update`, `finish`, `fix`, or `verify`.

</details>

### Limits & edge cases

- Input is read one line at a time. Put each bullet, action, or decision on its own line.
- The extractor cleans bullets, numbering, and checkbox prefixes, but it does not parse nested agendas or tables.
- It does not extract due dates, priorities, transcript speakers, or implicit commitments. Those require semantic/LLM handling or a separate purpose-built parser.
- If no action item or decision is found, the tool reports a clear error instead of returning an empty list.
