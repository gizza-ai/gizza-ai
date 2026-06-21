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
