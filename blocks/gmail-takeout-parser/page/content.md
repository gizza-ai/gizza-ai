## About this tool

Google Takeout hands you your Gmail as one big **`.mbox`** file — thousands of
raw email messages stacked in a single text blob. That's great for backups and
terrible for actually *reading* your mail history. This tool turns that export
into a clean table you can open in a spreadsheet: **one row per message**, with a
standardized date, the **from / to / cc** addresses, the subject, the **Gmail
labels** each message carried, and its message-id.

Paste the contents of an `.mbox` file (or even a single message), pick **CSV**
or **JSON**, and you get the table back instantly. CSV opens straight in Excel,
Numbers or Google Sheets; JSON is ready for a script or automation. Turn on
**Include a body snippet column** to add a short plain-text preview of each
message, and use the **snippet length** slider to decide how much of the body to
keep (set it to `0` for the full body).

Everything runs **in your browser** — your mail is parsed locally with a
pure-Rust email parser and never uploaded anywhere. The Gmail **labels** are read
from the `X-Gmail-Labels` header that Takeout writes, so the folder/label
structure you set up in Gmail survives the export.

## FAQ

<details>
<summary>Where do I get the .mbox file?</summary>

Go to **Google Takeout** (takeout.google.com), select **Mail**, and export. You
get a `.zip` containing one `.mbox` file (often named `All mail Including Spam
and Trash.mbox`). Open it in a text editor and paste its contents here — or paste
just the portion you want to convert.

</details>

<details>
<summary>Are the Gmail labels kept?</summary>

Yes — that's the point. Takeout records each message's labels in an
`X-Gmail-Labels` header, and this tool reads it into a **`labels`** column. A
message tagged `Inbox`, `Important` and `Work` shows all three, comma-separated,
in that column.

</details>

<details>
<summary>What columns does the table have?</summary>

`date` (standardized to ISO-8601 / RFC 3339), `from`, `to`, `cc`, `subject`,
`labels`, and `message_id`. If you enable the body snippet, a `snippet` column is
added at the end. CSV output is RFC-4180 quoted, so fields containing commas —
like a multi-label list — are safely wrapped in quotes.

</details>

<details>
<summary>Is my email uploaded anywhere?</summary>

No. Parsing happens entirely in your browser with WebAssembly. Nothing is sent
to a server, so even a private mailbox stays on your machine.

</details>

<details>
<summary>Can it handle my whole multi-gigabyte archive?</summary>

This tool takes **pasted** text, so it's best for a mailbox — or a slice of one —
that fits comfortably in browser memory. A full multi-GB Takeout archive is too
large to paste; split it or convert the parts you need. Attachment *files* aren't
extracted either — this is a message **table** tool.

</details>
