## About this tool

JMESPath is the JSON query language used by the AWS CLI `--query` flag. Paste a JSON document, enter an expression, and this tool evaluates it locally with a pure Rust engine. Use it to project fields, filter arrays, sort data, call built-in functions, or reshape objects before pasting the expression into scripts and CLI commands.

Worked examples:

```text
people[*].name
people[?age > `30`].{name: name, state: state}
sort_by(people, &age)[0].name
```

String literals use single quotes, JSON literals use backticks, and a missing match returns `null` rather than an error. Turn on raw output to print string results without JSON quotes and array items one per line.

Common built-ins supported by the underlying engine include `length`, `contains`, `starts_with`, `ends_with`, `sort`, `sort_by`, `max_by`, `min_by`, `join`, `keys`, `values`, `reverse`, `sum`, `avg`, `map`, `to_string`, and `to_number`.

Limits and edge cases: the page does not fetch remote URLs or contact AWS; it evaluates only the JSON you paste. Large documents must fit in browser memory. Syntax errors and type errors are reported separately from invalid JSON so you can tell which side needs fixing.

## FAQ

<details>
<summary>How is JMESPath different from JSONPath?</summary>

JSONPath focuses on selecting paths from a JSON tree. JMESPath also selects data, but it has a standardized expression language for projections, filters, pipes, functions, and object/list construction. It is the syntax used by `aws --query`.

</details>

<details>
<summary>What does raw output do?</summary>

Raw output is useful when your expression returns strings. A string result is printed without JSON quotes, and a top-level array is printed one item per line. Objects, numbers, booleans, and null still render as JSON-compatible text.

</details>

<details>
<summary>Why did my expression return null?</summary>

`null` is a valid JMESPath result. It usually means the path did not exist or a filter matched no object in the place you expected. Try projecting a smaller expression first, such as `people` or `people[*]`, then add filters one at a time.

</details>

<details>
<summary>Does this upload my JSON?</summary>

No. The evaluator runs as WebAssembly in your browser for the page surface, and the CLI/chat surfaces use the same local core. The tool has no live price, cloud, or account connection.

</details>

<details>
<summary>Can it generate code for my programming language?</summary>

No. This tool evaluates and debugs the expression itself. Once the expression works, paste it into the AWS CLI or a JMESPath library for your language.

</details>
