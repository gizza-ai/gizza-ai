## About this tool

Paste a GraphQL query, mutation, subscription, fragment, or SDL schema and get a validated, consistently formatted document back. The formatter parses the GraphQL grammar before printing, so syntax errors include a line and column instead of silently rearranging invalid text.

Use the indent selector for two spaces, four spaces, eight spaces, or tabs. Choose **minify** when you need a compact payload for examples or tests, enable **sort fields** for stable diffs, and enable **remove comments** when you want formatted output without `#` notes.

Worked example:

Input:

```graphql
query Hero($episode: Episode = JEDI) { hero(episode: $episode) { name friends { name } } }
```

Output with the default settings:

```graphql
query Hero($episode: Episode = JEDI) {
  hero(episode: $episode) {
    name
    friends {
      name
    }
  }
}
```

Limits and edge cases: input is capped at about 500 KB, nesting is capped at 64 levels, minify mode always removes comments, and the parser focuses on standard GraphQL executable documents and SDL definitions rather than vendor-specific non-GraphQL template wrappers.

## FAQ

<details>
<summary>Does this validate the GraphQL syntax?</summary>

Yes. The tool lexes and parses the document before printing it. If the source is not valid GraphQL, the output shows a syntax error with a line and column so you can jump to the problem.

</details>

<details>
<summary>Can it format schema definition language as well as queries?</summary>

Yes. It handles operations, fragments, schema definitions, object and input types, enums, unions, scalars, directives, descriptions, and common SDL extensions.

</details>

<details>
<summary>What does sort fields change?</summary>

It sorts selection fields recursively and sorts object/input fields in SDL blocks. That is useful for stable generated diffs, but leave it off when field order is meaningful for human review.

</details>

<details>
<summary>Does minify preserve comments?</summary>

No. GraphQL comments are ignored tokens, so minify mode drops them. If you want readable output with comments removed, use format mode with **Remove comments** enabled.

</details>
