## About this tool

JSON Assertion Runner is a small contract-test runner for JSON payloads. Paste an API response, fixture, or event body, then write assertions with JSONPath selectors and simple operators. The output is a pass/fail report that names the failing rule, the matched location, and the actual value when a comparison fails.

Use it when a full test framework is too heavy but you still need repeatable checks: smoke tests for API samples, fixture validation before a demo, webhook payload checks, or QA notes that should be executable instead of prose.

## Assertion syntax

The line DSL is one assertion per line:

```text
$.status equals ok
$.user.id matches ^u-\d+$
$.items length 2
$.items[*] count 2
$.items[*].price gt 0
$.items[*].price lte 100
$.user.roles contains admin
$.user.name type string
$.optional not_exists
```

Blank lines and `# comments` are ignored. The same rules can be supplied as a JSON array of objects with `path`, `op`, and optional `expected` fields (plus an optional `name` that is echoed in the report) when you want to generate checks from another script.

Supported operators include `exists`, `not_exists`, `equals`, `not_equals`, `type`, `gt`, `gte`, `lt`, `lte`, `range`, `contains`, `not_contains`, `matches`, `count`, `length`, `empty`, and `not_empty`.

## Worked example

Payload:

```json
{"status":"ok","user":{"id":"u-42","roles":["admin","dev"]},"items":[{"sku":"A1","price":9.5},{"sku":"B2","price":250}]}
```

Assertions:

```text
$.status equals ok
$.user.id matches ^u-\d+$
$.user.roles contains admin
$.items length 2
$.items[*].price gt 0
```

Report:

```text
PASS — 5 of 5 assertions passed, 0 failed

PASS  #1  $.status equals "ok"
PASS  #2  $.user.id matches ^u-\d+$
PASS  #3  $.user.roles contains "admin"
PASS  #4  $.items length 2
PASS  #5  $.items[*].price gt 0
```

Swap the last rule for `$.items[*].price lte 100` and the run fails with the value that broke it:

```text
FAIL — 4 of 5 assertions passed, 1 failed

FAIL  #5  $.items[*].price lte 100
          at $['items'][1]['price']: expected <= 100, got 250
```

## Limits and edge cases

- Up to 200 assertions per run; anything more is rejected rather than truncated.
- Nothing is fetched: the payload is whatever you paste. Everything runs locally in your browser.
- JSONPath selectors are evaluated against one JSON value at a time; newline-delimited JSON must be split before running checks.
- In the line DSL the path is the first whitespace-delimited token, so a path containing spaces (`$['order id']`) has to go through the JSON rules format instead.
- `equals` compares JSON values, so unquoted DSL tokens are parsed as JSON when possible (`true`, `42`, `null`) and otherwise treated as strings.
- `type integer` is stricter than `type number`; `9.5` is a number but not an integer.
- `contains` works on arrays (membership), strings (substring), and objects (key). Use `count` for how many nodes a JSONPath selected — `$.items count 2` selects one array, `$.items[*] count 2` selects two items — and `length` for the size of a matched array, string, or object.
- A value comparison must hold for every matched node, and a path that matches nothing fails; use `not_exists` when absence is what you mean.
- Regexes use Rust regular expressions. They are not JavaScript regex literals, so write `^u-\d+$` rather than `/^u-\d+$/`.
- Turning off case sensitivity affects string equality, string/array contains checks, and regex matching.

## FAQ

<details>
<summary>Is this the same as JSON Schema validation?</summary>

No. JSON Schema validates a document shape with a schema language. This tool runs explicit assertions against JSONPath selections. It is useful for spot checks such as “the status is ok”, “there are two items”, or “every price is below 100”.

</details>

<details>
<summary>Can one JSONPath match multiple values?</summary>

Yes. Value comparisons must pass for every matched value. For example, `$.items[*].price lte 100` fails if any item price is over 100 and reports the first failing location.

</details>

<details>
<summary>What is the difference between <code>count</code> and <code>length</code>?</summary>

`count` asserts how many nodes the JSONPath selected, and `length` asserts the size of each node it selected. For a two-item array, `$.items count 1` and `$.items length 2` both pass, while `$.items[*] count 2` passes because the wildcard selects each item separately. When a count check fails on a single matched array, object, or string, the report says so and points at `length`.

</details>

<details>
<summary>When should I use JSON rules instead of the line DSL?</summary>

Use JSON rules when another tool or script generates the assertions. The line DSL is quicker to type by hand; the JSON array form is easier to serialize and review in automation.

</details>

<details>
<summary>Does the tool fetch URLs or call an API?</summary>

No. It only evaluates the JSON payload you paste. Fetch the response with curl, your browser, or another tool first, then paste the JSON here.

</details>
