## About mail merge

This tool does a classic **mail merge**: it fills a text or markdown **template**
once for every row of a **CSV**, then joins the results. Each `{{Column}}`
placeholder in the template is replaced by that row's value for the matching CSV
header column, so one small template plus a spreadsheet of data becomes a batch
of personalized letters, emails, labels, reminders, or messages.

Everything runs locally in your browser. Your template and data are never
uploaded to a server, and it works offline.

### How it works

1. **Template** — write your text and drop `{{name}}` style placeholders where
   the per-row values should go. The name inside the braces must match a column
   header in your CSV.
2. **CSV data** — paste rows where the **first line is the header** (the column
   names) and each following line is one record.
3. Pick the placeholder **style**, the CSV **delimiter**, and a **separator** to
   put between the generated documents, then run.

### Worked example

Template:

```
Hi {{name}},

Your invoice for ${{amount}} is due on {{due}}.

Thanks!
```

CSV:

```
name,amount,due
Alice,120,2026-08-01
Bob,90,2026-08-05
```

Output (with the default `---` divider between rows):

```
Hi Alice,

Your invoice for $120 is due on 2026-08-01.

Thanks!

---

Hi Bob,

Your invoice for $90 is due on 2026-08-05.

Thanks!
```

### Options

- **Placeholder style** — `{{column}}` (double curly, default), `{column}`
  (single curly), or `<<column>>` (double angle). Pick whichever doesn't clash
  with literal braces in your template.
- **CSV delimiter** — comma (default), semicolon (common in European exports),
  or tab (TSV).
- **If a placeholder column is missing** — *replace with nothing* (the usual
  mail-merge behavior), *keep the `{{placeholder}}` text* (handy for catching a
  typo in a field name), or *show an error*.
- **Case-insensitive matching** — on by default, so `{{First Name}}` matches a
  header called `first name`.
- **Separator between documents** — a `---` rule (default), a blank line, a
  single newline, a page break (form feed), or none.

### Limits and edge cases

- Up to **1000 data rows** per merge (an in-browser memory bound). Split larger
  lists into batches.
- A placeholder for a column that **exists but is empty** on a given row always
  renders as an empty string, regardless of the missing-column setting.
- A row with **fewer fields** than the header treats the absent trailing cells as
  empty.
- Quoted CSV fields (`"a, b"`) may contain the delimiter and newlines.
- This is **plain named substitution** — there are no loops, conditionals, or
  expressions. For Handlebars/Mustache logic against a single JSON object, use
  the render-template tool instead.

### FAQ

<details>
<summary>What placeholder syntax should I use?</summary>

By default, wrap a column name in double curly braces: `{{name}}`. If your text
already contains literal braces, switch the **placeholder style** to `{column}`
or `<<column>>` so your merge fields don't clash with them.

</details>

<details>
<summary>Does the first CSV row have to be a header?</summary>

Yes. The first line is read as the column names, and every line after it is a
data row that produces one output. Placeholders are matched to those header
names.

</details>

<details>
<summary>What happens if a placeholder's column isn't in the CSV?</summary>

It depends on the **If a placeholder column is missing** setting: *replace with
nothing* drops it, *keep the placeholder text* leaves `{{that}}` in place so you
can spot the mismatch, and *show an error* stops the merge and names the missing
column. Note a column that exists but is blank for a row always renders empty.

</details>

<details>
<summary>My data uses semicolons or tabs — is that supported?</summary>

Yes. Set the **CSV delimiter** to semicolon (common in European spreadsheet
exports) or tab (for TSV). Quoted fields containing the delimiter or newlines are
handled correctly.

</details>

<details>
<summary>Can I do loops or if/else in the template?</summary>

No — mail merge is plain `{{column}}` substitution repeated per row. For
Handlebars/Mustache logic (`{{#each}}`, `{{#if}}`, nested paths) against a single
JSON object, use the render-template tool.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The merge runs entirely in your browser using WebAssembly. Your template and
CSV never leave your device, and the page works offline once loaded.

</details>
