## About this tool

Render a **Jinja2** (Jinja) template against JSON or YAML data, entirely in your
browser. Paste your template, supply your data, and get the rendered output
instantly — nothing is uploaded to a server.

### What's supported

- **Variable substitution** — `{{ name }}` is replaced by the matching value in your data.
- **Nested paths** — `{{ user.profile.email }}` reaches into nested objects.
- **Loops** — `{% for item in items %}- {{ item }}\n{% endfor %}` repeats a block for each element.
- **Conditionals** — `{% if admin %}…{% elif staff %}…{% else %}…{% endif %}`.
- **Filters** — `{{ name | upper }}`, `{{ items | join(', ') }}`, `{{ price | round(2) }}`, and more.
- **JSON or YAML data** — choose the data format, or leave it on *auto* to detect it.
- **Strict mode** — turn on *Strict* to make a reference to a missing variable an
  error instead of rendering empty.

Output is not HTML-escaped, so the tool is equally useful for generating emails,
config files, code, or any other text.

### Example

Template:

```
Hello {{ name }}!
{% for item in items %}- {{ item }}
{% endfor %}
```

Data (JSON):

```json
{"name":"Ada","items":["apples","bananas"]}
```

Output:

```
Hello Ada!
- apples
- bananas
```

### Jinja2 vs. Handlebars / Mustache

Jinja2 uses `{% ... %}` for statements (loops, conditionals) and `{{ ... }}` for
expressions, with a rich filter and expression system — a different language from
the Handlebars/Mustache `{{#each}}` style. Pick the renderer that matches the
template syntax you have.
