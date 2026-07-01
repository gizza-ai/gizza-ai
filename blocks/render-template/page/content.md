## About this tool

Render a **Handlebars** or **Mustache** template against JSON data, entirely in
your browser. Paste your template on the left, your data as JSON, and get the
rendered output instantly — nothing is uploaded to a server.

### What's supported

- **Variable substitution** — `{{name}}` is replaced by the matching value in your data.
- **Nested paths** — `{{user.profile.email}}` reaches into nested objects.
- **Loops** — `{{#each items}}- {{this}}\n{{/each}}` repeats a block for each array element.
- **Conditionals** — `{{#if admin}}…{{else}}…{{/if}}` renders a block based on a value.
- **Strict mode** — turn on *Strict* to make a reference to a missing variable an
  error instead of rendering empty.

Handlebars is a superset of Mustache, so the same `{{variable}}` syntax works for
both engines. Output is not HTML-escaped, so the tool is equally useful for
generating emails, config files, code, or any other text.

### Example

Template:

```
Hello {{name}}!
{{#each items}}- {{this}}
{{/each}}
```

Data:

```json
{"name":"Ada","items":["apples","bananas"]}
```

Output:

```
Hello Ada!
- apples
- bananas
```

## FAQ

<details>
<summary>Why isn't the output HTML-escaped?</summary>

Escaping is deliberately disabled for both engines, so `{{var}}` emits the
value verbatim — that's what you want when generating emails, config files, or
code. If you're producing HTML from untrusted data, escape the values yourself
before putting them in the JSON.

</details>

<details>
<summary>What happens when the template references a variable my data doesn't have?</summary>

By default it renders as an empty string, so `[{{missing}}]` becomes `[]`.
Turn on **Strict** to make any missing variable a hard error instead — useful
for catching typos in field names.

</details>

<details>
<summary>Are partials and custom helpers supported?</summary>

No — `{{> partial}}` includes and user-defined helpers aren't available. The
supported feature set is variable substitution, nested paths like
`{{user.profile.email}}`, `{{#each}}` loops (with `{{this}}`), and
`{{#if}}…{{else}}…{{/if}}` conditionals.

</details>

<details>
<summary>Can I leave the data field empty?</summary>

Yes — empty data is treated as `{}`, which renders fine in lenient mode.
Anything non-empty must be valid JSON (an object or array), otherwise you get
a parse error rather than a silent blank result.

</details>
