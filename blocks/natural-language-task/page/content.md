## Plain English in, todo.txt out

Type a task the way you'd say it out loud and this tool rewrites it as a single
[todo.txt](https://github.com/todotxt/todo.txt) line — the plain-text task format you can keep in any
editor, sync with Dropbox, and open in dozens of apps. It reads the priority, the `+project`, the
`@context` and the deadline straight out of your sentence, so you never have to remember the syntax.

Everything runs locally in your browser. There is no LLM, no account and no upload — the same plain
sentence always produces the same line, because the parsing is deterministic.

### Worked example

Input:

```
Call the plumber urgent tomorrow +house @phone
```

With the reference date set to **2026-07-28**, the output is:

```
(A) call the plumber +house @phone due:2026-07-29
```

- **urgent** → the leading `(A)` priority, and the word is removed from the title.
- **tomorrow** → `due:2026-07-29`, anchored on the reference date.
- **+house** and **@phone** → kept exactly as written.

Paste several lines to convert a whole brain-dump at once — one todo.txt line comes back per input
line.

### What it recognises

- **Priority** — `urgent`, `asap`, `critical`, `important`, `emergency`, `high priority` → `(A)`;
  `low priority`, `someday`, `whenever`, `minor` → `(C)`; Todoist-style `p1`–`p4` → `(A)`–`(D)`; an
  explicit `(A)`–`(D)` you type is kept.
- **Due date** — `today`, `tonight`, `tomorrow`, `day after tomorrow`, weekday names (with `next`/
  `this`), `next week`/`next month`, `in 3 days`, `in 2 weeks`, ISO dates like `2026-08-01`, `M/D`,
  and `March 5, 2027`.
- **Projects & contexts** — any `+project` or `@context` you type inline is preserved; you can also
  set a default project/context to append to lines that don't have one.
- **Creation date** — optionally stamps the reference date as the todo.txt creation date, e.g.
  `(A) 2026-07-28 call bob`.

## FAQ

<details>
<summary>What is the todo.txt format?</summary>

todo.txt is a simple, future-proof convention for one task per line in a plain text file. A line can
carry a priority in parentheses at the start (`(A)` is highest), an optional creation date, the task
text, `+project` and `@context` tags, and `key:value` pairs such as `due:2026-08-01`. Because it's
just text, it works in any editor and in a wide range of task apps and command-line tools.

</details>

<details>
<summary>How are relative dates like "tomorrow" or "next Friday" resolved?</summary>

They're measured from the **reference date** field, which defaults to today. So with the reference
date on a Tuesday, "next Friday" resolves to that week's Friday and "in 3 days" adds three days. Set
the reference date explicitly to get reproducible output regardless of when you run the tool — handy
for tests and for backdating a brain-dump.

</details>

<details>
<summary>Can I convert a whole list at once?</summary>

Yes. Put one task per line in the task box and each non-blank line becomes its own todo.txt line.
Leading bullets, checkboxes (`- [ ]`) and numbering are stripped automatically, so you can paste a
Markdown checklist or a numbered list straight in.

</details>

<details>
<summary>What if my task has no date or no priority?</summary>

Nothing is invented. A line only gains a `due:` date when it actually contains a recognised date
phrase, and a priority only when a priority cue is present. A plain "buy milk" stays "buy milk"
(optionally with a creation date and any default project/context you set).

</details>

<details>
<summary>How do I turn off date or priority detection?</summary>

Uncheck **Detect due date** to keep date words in the title and add no `due:` key, or uncheck
**Detect priority** to leave the title verbatim with no leading `(A)`. This is useful when a task
legitimately mentions a day name or the word "important" that you don't want interpreted.

</details>

## Limits & notes

- **Date-only due dates.** Output uses `due:YYYY-MM-DD`; clock times (for example "at 3pm") are not
  added to the `due:` key.
- **First date and first priority per line.** If a line contains more than one date phrase or
  priority cue, the earliest one is used and the rest stay in the text.
- **Priority range.** Priorities are clamped to `(A)`–`(D)`; an explicit letter beyond `D` is folded
  to `(D)`.
- **Single-word tags.** Default `+project`/`@context` values have their spaces replaced with hyphens
  so they stay a single todo.txt token (e.g. `Summer Vacation` → `+Summer-Vacation`).
- **Deterministic & private.** All parsing happens in your browser; the same input and reference
  date always produce the same output.
