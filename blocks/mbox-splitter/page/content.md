## About this tool

An mbox file is one long text file holding many emails end to end. Mail clients
and export services — Thunderbird, Apple Mail, Gmail Takeout, mailing-list
archives — all hand you a single `.mbox`, which is awkward when you only want
one message, or when the thing you need to open the mail with expects one file
per message. This tool splits the archive back into the individual messages it
contains, each a ready-to-save `.eml` with a suggested filename.

Messages are separated the classic way: a `From ` postmark line at the start of
a line. The space after `From` is what distinguishes the separator from a
`From:` header, so header lines never split a message by mistake. Each message
is then sliced out **verbatim** — headers, MIME structure, and base64
attachments are copied byte for byte, never re-serialized — so what you save is
a faithful copy of the original mail. The postmark line itself is dropped by
default, because a `.eml` file is a bare RFC 5322 message; turn on **Keep the
From postmark line** if you want to reassemble an mbox later.

Four things you can ask for:

- **Every message as .eml text** — each message under a
  `===== 001-name.eml (N bytes) =====` header, so you can copy one out or save
  the whole thing.
- **Index of what is inside** — a numbered table of filename, date, sender,
  subject, and size, with no bodies. Useful for checking an archive before
  doing anything with it.
- **JSON records** — `{ index, filename, subject, from, date, bytes, eml }` per
  message, for scripting.
- **One raw message only** — combine with a message number to get exactly one
  `.eml` and nothing else. This is the mode to use with the Download link.

Filenames follow whichever scheme you pick — numbered, subject, date, or
Message-ID — and always keep the `001-` index prefix so the original archive
order and filename uniqueness survive. Subjects written as RFC 2047 encoded
words (`=?utf-8?q?Caf=C3=A9?=`) are decoded first and then slugged to portable
lowercase ASCII.

Exporters escape body lines that begin with `From ` by writing `>From ` (the
mboxo/mboxrd convention). **Undo >From body quoting** is on by default so bodies
match the message as sent; switch it off if you want the archive bytes exactly
as they were stored.

Everything runs locally in your browser as WebAssembly — the archive is never
uploaded.

**Limits:** one run splits at most 2000 messages, and input arrives as text
through the field above, so a multi-gigabyte export should be cut down first.
The tool returns text, not a ZIP of files: to save a single message, choose
**One raw message only** with a message number and use the Download link, then
rename the downloaded file to `.eml`.

### Worked example

Input:

```text
From alice@example.com Mon Sep 03 10:00:00 2018
From: Alice <alice@example.com>
Subject: Quarterly report
Message-ID: <a1@example.com>
Date: Mon, 3 Sep 2018 10:00:00 +0000

Numbers attached.

From bob@example.com Mon Sep 03 11:30:00 2018
From: Bob <bob@example.com>
Subject: Lunch?
Message-ID: <b2@example.com>
Date: Mon, 3 Sep 2018 11:30:00 +0000

One o'clock?
```

With **Index of what is inside** and date filenames, the output is:

```text
2 message(s)

  1. 001-2018-09-03-1000.eml
     date:    2018-09-03T10:00:00+00:00
     from:    alice@example.com
     subject: Quarterly report
     size:    141 bytes

  2. 002-2018-09-03-1130.eml
     date:    2018-09-03T11:30:00+00:00
     from:    bob@example.com
     subject: Lunch?
     size:    124 bytes
```

Switching to **One raw message only** with message `2` returns Bob's message on
its own, starting at `From: Bob <bob@example.com>` — no postmark, no other mail.

## FAQ

<details>
<summary>How does the tool know where one message ends and the next begins?</summary>

It splits on the mbox postmark: a line starting with `From ` — the word `From`
followed by a space — at column 0. That is the separator every mbox writer
emits. A `From:` header has a colon instead of a space, so it is never mistaken
for a separator. If your text has no postmark at all, it is treated as a single
message, which means a lone `.eml` pasted in still works.

</details>

<details>
<summary>Are attachments and formatting preserved?</summary>

Yes. Each message is copied out of the archive verbatim, including MIME
boundaries, `Content-Type` headers, and base64-encoded attachment parts. The
tool only reads the Subject, Date, and Message-ID headers, and only to build the
suggested filenames — it never rewrites the message. Saving one of the output
blocks as a `.eml` gives you a file your mail client can open with attachments
intact.

</details>

<details>
<summary>Can I download every message as separate files or a ZIP?</summary>

Not from this page — it produces text, so a multi-file download has nowhere to
go. What you can do is choose **One raw message only**, set the message number,
and use the Download link to save that message (rename it from `.txt` to
`.eml`). For a whole archive, the `.eml` text mode prints every message under a
labelled header, which you can save once and split with a script, or run the
command-line version once per message and redirect the output.

</details>

<details>
<summary>What does the `>From ` option do?</summary>

Because a body line beginning with `From ` would look like a message separator,
mbox writers escape it as `>From ` (and an already-escaped `>From ` becomes
`>>From `). Leaving **Undo >From body quoting** on strips one level of that
quoting so the body reads as it was sent. Turn it off when you want to see the
stored bytes unchanged — for example when comparing against the original file.

</details>

<details>
<summary>Should I keep the postmark line?</summary>

Keep it off if you are producing `.eml` files: an `.eml` is a bare RFC 5322
message and the postmark (`From sender Mon Sep 03 10:00:00 2018`) is an mbox
container artifact that some mail clients display as junk. Turn it on when you
want each piece to be a valid one-message mbox — for instance if you plan to
concatenate a subset back into a smaller archive.

</details>

<details>
<summary>Why did I get a message-count error?</summary>

Two caps guard the browser. An archive with more than 2000 messages is rejected
outright — split the file first. And asking for a single message number larger
than the archive holds is an error that tells you how many messages there
actually are, so you can pick a valid one; use `0` to get every message.

</details>
