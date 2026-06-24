## What this tool does

Turn a plain list of fields into clean, **accessible HTML form markup** — ready
to paste into any page. Every field gets a properly bound `<label for>`, a
slugified `name`/`id`, and the right validation attributes (`required`,
`type="email"`, `type="number"`, and so on). Nothing is sent to a server — it
runs locally in your browser, works offline, and needs no sign-up.

## Field syntax

Describe **one field per line**. Each line has up to three parts separated by
`|`:

```
Label | type | options
```

- **Label** — the visible label. End it with `*` to make the field **required**
  (e.g. `Email*`). The label also becomes the input's `name`/`id`, slugified
  (`Full Name` → `full-name`).
- **type** *(optional, default `text`)* — the control to render (see below).
- **options** *(optional)* — for `select`, `radio` and `checkboxes`, a
  comma-separated list of choices. For every other type it becomes the input's
  **placeholder**.

Blank lines and lines starting with `#` are ignored.

## Supported field types

| Type | Renders |
| --- | --- |
| `text` (default) | a single-line text input |
| `email` | `<input type="email">` (browser email validation) |
| `password` | `<input type="password">` |
| `number` | `<input type="number">` |
| `tel` | `<input type="tel">` |
| `url` | `<input type="url">` |
| `date` / `time` | a native date / time picker |
| `textarea` | a multi-line `<textarea>` |
| `select` | a `<select>` dropdown — give the options |
| `radio` | a group of radio buttons sharing one `name` — give the options |
| `checkbox` | a single checkbox (label after the box) |
| `checkboxes` | a group of checkboxes using an array `name[]` — give the options |

## Options

- **Method** — the form's `method` attribute, `post` (default) or `get`.
- **Action URL** — the `action` where the form submits (optional).
- **Submit button text** — the label on the submit button.
- **Include a `<style>` block** — when on, the output starts with a small
  stylesheet and the form gets the `gizza-form` class so it looks good on its
  own. Turn it off for bare, class-free markup you can style yourself.

## Example

Input:

```
Full Name*
Email* | email
Topic | select | Sales, Support, Other
Message | textarea | How can we help?
Subscribe to the newsletter | checkbox
```

produces a complete `<form>` with a required name and email, a topic dropdown, a
message textarea, and an opt-in checkbox — each with its own label and
validation.

## FAQ

**Is it accessible?** Yes. Every input has a `<label for>` bound to its `id`,
radio/checkbox groups share a name and have a group label, and required fields
carry the `required` attribute plus a visible `*` marker.

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Can I style it myself?** Turn off the `<style>` block to get plain markup with
clean `name`/`id` attributes, then apply your own CSS.

**Where does the form submit?** Wherever you set the **Action URL**. With no
action, the browser submits back to the current page — wire it to your own
endpoint or handler.
