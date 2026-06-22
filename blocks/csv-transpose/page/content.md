## About this tool

**CSV transpose** flips a CSV on its diagonal: rows become columns and columns
become rows. The first column of the output is what used to be the header row, so:

```
name,age          name,Ada,Bo
Ada,36     ->     age,36,40
Bo,40
```

Ragged rows are padded with empty cells so the result is rectangular. Transposing
twice gives you the original back. Works with `,` / tab / `;` / `|` delimiters.

### Privacy

Everything runs **in your browser** via WebAssembly — your CSV is never uploaded.
Also available from the [gizza CLI](/) and in chat.

### Common uses

- Turn a wide table into a tall one (or vice versa) for a different tool.
- Put each record in a column for side-by-side comparison.
