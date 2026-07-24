## About this tool

Paste an mbox mailbox export and this tool removes duplicate messages by their
`Message-ID` header. It is useful after merging mail archives, importing the
same mailing-list dump twice, or cleaning a support mailbox before sharing a
smaller archive.

The input is treated as classic mbox text: messages are separated by `From `
postmark lines at the start of a line. For each message, the header section is
scanned for `Message-ID`. Values are normalized by trimming whitespace and one
surrounding pair of angle brackets, so `<abc@example.com>` and
` abc@example.com ` compare the same. By default comparison is case-sensitive,
which matches RFC 5322; turn on case-insensitive matching only when an exporter
has changed the case of otherwise-identical IDs.

You can keep the **first** or **last** copy of each duplicated ID. Surviving
messages are emitted in their original order and preserved verbatim, including
postmark lines, headers, body text, and blank-line separators. Messages with no
`Message-ID` are kept by default because two drafts without IDs may still be
different messages; switch that option to **drop** if you want to remove them.

Everything runs locally in your browser through WebAssembly. The original mbox
is never uploaded or modified; the output is a fresh de-duplicated mbox you can
copy or download.

### Worked example

Input:

```text
From a@example.com Mon Sep 03 10:00:00 2018
Message-ID: <same@example.com>

first copy

From b@example.com Mon Sep 03 11:00:00 2018
Message-ID: <other@example.com>

unique message

From c@example.com Mon Sep 03 12:00:00 2018
Message-ID: <same@example.com>

duplicate copy
```

With **keep first**, the output keeps `first copy` and `unique message`, and
removes `duplicate copy` because it has the same normalized `Message-ID` as the
first message.

## FAQ

<details>
<summary>What counts as a duplicate?</summary>

Two messages are duplicates when their `Message-ID` header matches after
trimming whitespace and one surrounding `<...>` pair. The tool does not compare
subject, sender, body text, attachments, or dates; that avoids fuzzy matches and
keeps this tool focused on the standard mail identifier.

</details>

<details>
<summary>Should I keep the first or the last copy?</summary>

Use **keep first** when the first archive is your trusted source and later
imports are accidental repeats. Use **keep last** when later copies are more
likely to contain the message version you want. Either way, the surviving
messages stay in their original order in the output.

</details>

<details>
<summary>Why are messages without Message-ID kept by default?</summary>

Drafts, malformed exports, and some generated messages may not have a
`Message-ID`. Without an ID there is no safe Message-ID-based key, so the default
is to keep every ID-less message instead of merging distinct mail by mistake. If
you know ID-less messages are junk in your archive, choose **drop**.

</details>

<details>
<summary>Does the tool change line endings or rewrite messages?</summary>

No. Surviving message blocks are concatenated verbatim from the input, including
postmark lines, headers, bodies, blank lines, and existing CRLF or LF endings.
Only whole duplicate message blocks are removed.

</details>

<details>
<summary>Is the mbox uploaded anywhere?</summary>

No. The parser and de-duplicator run entirely in your browser as WebAssembly.
Your mailbox text stays on your machine; the output is generated locally.

</details>
