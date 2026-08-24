## About this tool

Use this TOML formatter when you want a clean, validated config file without turning it into a lossy value dump. It parses the document first, reports invalid TOML with line and column context, then emits normalized spacing, table ordering, key ordering and array layout according to the options you choose.

Unlike simple value-model formatters, this formatter keeps own-line comments, end-of-line comments, scalar literal spelling such as `0xFF` or `1_000_000`, and literal strings wherever the requested layout can still represent them. That makes it suitable for Cargo.toml, pyproject.toml, taplo.toml and other hand-edited configuration files.

### Worked example

Input:

```toml
# Package metadata
[package]
version="0.1.0"
name='demo' # literal spelling kept
features=["cli","web","docs"]
```

With the defaults, the output becomes:

```toml
# Package metadata
[package]
version = "0.1.0"
name = 'demo' # literal spelling kept
features = ["cli", "web", "docs"]
```

Choose `sort_keys=asc` to alphabetize entries within each table, `array_style=expand` for one item per line, `spacing=compact` for `key=value`, or disable `keep_comments` when you intentionally want a comment-free file.

## Limits and edge cases

- Input must be valid TOML. Invalid input returns an error and no formatted output.
- `array_style=collapse` cannot preserve comments inside arrays because a single-line array cannot safely contain `#` comments.
- Output uses LF line endings and a trailing newline.
- This is a one-document formatter. For many files, run the CLI command in a shell loop.

## FAQ

<details>
<summary>Does this preserve TOML comments?</summary>

Yes for normal own-line comments and end-of-line comments on entries or table headers. Comments inside arrays are preserved when the array remains expanded; forcing `array_style=collapse` drops those inner comments because they cannot be represented safely on one line.

</details>

<details>
<summary>Will formatting change numeric or string values?</summary>

The parser validates values, but scalar literals are emitted from the syntax tree rather than reconstructed from a generic value model. That means spellings such as `0xFF`, `1_000_000`, date offsets and literal strings are kept instead of being rewritten.

</details>

<details>
<summary>What does the key sorting option sort?</summary>

`sort_keys=asc` and `sort_keys=desc` sort entries within each table and keys inside inline tables. They do not reorder array values because array order is often meaningful in TOML configuration.

</details>

<details>
<summary>Why is the default indent set to zero?</summary>

Most common TOML files, including Cargo.toml and pyproject.toml, keep entries flat under each table header. The default follows that convention; set the indent slider above zero if you prefer nested table entries to be indented.

</details>
