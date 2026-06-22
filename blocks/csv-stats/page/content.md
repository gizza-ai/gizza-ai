## About this tool

**CSV stats** gives you a quick `describe()`-style summary of a CSV: for each
column you get the value **count**, **empty-cell** count, and number of **distinct**
values — and for columns whose values are all numbers, the **min**, **max**,
**mean**, and **sum**.

Paste a CSV (with or without a header) and pick the delimiter (`,` / tab / `;` /
`|`). Great for sizing up a dataset before you analyse it.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat (which return the stats as
structured JSON).

### Common uses

- Spot the range and average of a numeric column at a glance.
- Find columns with missing (empty) cells.
- Check how many distinct values a category column has.
