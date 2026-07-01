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

## FAQ

<details>
<summary>Why does a missing variable just render as nothing?</summary>

That's Jinja's default lenient behavior — an undefined reference like
`{{ typo_name }}` produces an empty string. Turn on **Strict** mode and the same
reference becomes an error instead, which is the safer setting when you're
debugging a template that "mysteriously" outputs blanks.

</details>

<details>
<summary>How does the JSON/YAML auto-detection work — can it guess wrong?</summary>

On *auto* the tool tries JSON first, then YAML. Because almost every JSON
document is also valid YAML, ambiguity is rare — but if your data is YAML that
happens to parse as something unintended, force the format to `yaml` (or `json`)
with the data-format option.

</details>

<details>
<summary>Is the output HTML-escaped?</summary>

No — what the template produces is exactly what you get, which makes the tool
just as useful for emails, config files, and code as for markup. If you're
generating HTML from untrusted data, apply escaping yourself (e.g. the
`| escape` filter) where needed.

</details>

<details>
<summary>Can I use Jinja2 filters and nested data?</summary>

Yes. Filters like `{{ name | upper }}`, `{{ items | join(', ') }}` and
`{{ price | round(2) }}` are supported, and dotted paths such as
`{{ user.profile.email }}` reach into nested objects and lists from your JSON or
YAML data.

</details>
