## About this tool

Timesheet to Invoice turns billable-hours notes into a formatted invoice. Paste one line per item using `description | hours`, `description | hours | rate`, `YYYY-MM-DD | description | hours`, or `YYYY-MM-DD | description | hours | rate`.

Hours can be decimal (`3.5`), hours/minutes (`2h 30m`), clock-style duration (`2:30`), or a start-end range (`09:00-12:30`, `9am-5pm`). The tool prices each line, optionally rounds billing time up to an increment, applies a discount before tax, computes a due date from payment terms, and renders Markdown, plain text, CSV, or JSON.

Example input:

```text
2026-08-03 | Landing page copy | 3.5
2026-08-04 | Bug fixes | 2h 30m
2026-08-05 | Client call | 09:00-10:15
```

At $120/hour, the example totals 7.25 hours, a $870.00 subtotal, and a $870.00 amount due before any tax or discount.

## Limits and edge cases

- Maximum pasted timesheet size is 1,000,000 bytes and 500 billable rows.
- Rows are split with `|` or tabs; commas are safe inside descriptions because CSV input is not required.
- Per-row rates override the default hourly rate.
- Billing increments round each row up after parsing. Use `0` to bill exact tracked time.
- A blank issue date omits automatic due-date calculation. A due date you enter manually always wins.
- This produces text invoices only. It does not create PDFs, accept payments, store client records, or send email.

## FAQ

<details>
<summary>Can I use different hourly rates on the same invoice?</summary>

Yes. Add a fourth field to a dated row or a third field to an undated row, such as `2026-08-03 | Rush support | 1.5 | 180`. Rows without their own rate use the default hourly rate.

</details>

<details>
<summary>How does rounding work?</summary>

The rounding setting is a billing increment in minutes. If you choose `15`, a 16-minute row bills as 30 minutes, while an exact 15-minute row stays at 15 minutes.

</details>

<details>
<summary>Can this merge repeated tasks?</summary>

Yes. Keep one line per entry, merge rows with the same description, or merge rows by service date. Merging happens before rounding so a grouped row rounds once.

</details>

<details>
<summary>Is this a legal invoice system?</summary>

No. It is a deterministic formatting helper. Review the output, add any legally required business or tax details, and keep your own accounting records.

</details>
