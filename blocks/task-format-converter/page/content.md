## About this tool

The **Task Format Converter** turns one task list into another common task-list format: todo.txt, Markdown checklists, JSON, or CSV. It parses each task into a shared model before writing the target format, so completion state, priority, `+projects`, `@contexts`, creation dates, due dates, completion dates, and extra `key:value` metadata survive the conversion.

Use it when you are moving tasks between a plain-text todo.txt workflow, a README or issue checklist, a spreadsheet, and a scriptable JSON pipeline. Set the input format explicitly for strict conversions, or choose **Auto-detect** to sniff JSON arrays/objects, Markdown checklist items, CSV header rows, and otherwise fall back to todo.txt.

## Worked example

Input todo.txt:

`(A) 2026-08-01 Draft release notes +work @desk due:2026-08-05`

Converted to Markdown checklist:

`- [ ] (A) Draft release notes +work @desk due:2026-08-05 created:2026-08-01`

The task text stays readable, and todo.txt's native creation date is preserved as a `created:` metadata tag because Markdown checklists do not have a built-in creation-date field.

## Limits and edge cases

- Markdown output uses `created:` and `completed:` metadata tags for dates that Markdown checkboxes cannot represent natively.
- CSV uses a fixed header: `text,done,priority,projects,contexts,created,due,completed,tags`.
- todo.txt priorities are uppercase single letters such as `(A)`.
- Auto-detect is intentionally simple; choose the source format explicitly if a list could be interpreted more than one way.

## FAQ

<details>
<summary>Which task formats are supported?</summary>

The converter supports todo.txt, Markdown checklists, JSON task objects, and CSV with task-oriented columns. JSON may be a single object or an array of objects.

</details>

<details>
<summary>What task fields are preserved?</summary>

It preserves task text, done state, uppercase priority, projects, contexts, created date, due date, completed date, and extra `key:value` metadata tags where the target format can represent them.

</details>

<details>
<summary>Why do dates become tags in Markdown?</summary>

Markdown checkboxes only encode checked or unchecked state. To avoid losing todo.txt creation/completion dates, the converter writes them as `created:` and `completed:` metadata tags in the checklist item text.

</details>

<details>
<summary>Can it synchronize with a task app?</summary>

No. This is an offline format converter for pasted text. It does not connect to Todoist, Things, OmniFocus, Google Tasks, or other external services.

</details>
