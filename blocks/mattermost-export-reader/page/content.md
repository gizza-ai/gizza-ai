## What this tool does

Paste the `import.jsonl` file from a Mattermost bulk export and get a readable transcript you can review, archive, search or hand to a migration/audit workflow. The conversion is deterministic and local: the JSON Lines file is parsed in your browser, display names are resolved from the export's user records, and channel posts, direct messages, replies, reactions and attachments are rendered without uploading the archive anywhere.

Mattermost's bulk export is built for imports, not humans. Each line is a separate object tagged with a `type` such as `version`, `team`, `channel`, `user`, `post`, `direct_channel` or `direct_post`. This tool indexes the metadata first, then renders the message records with UTC timestamps and friendly labels.

## Worked example

Input — a small Mattermost JSONL export:

```jsonl
{"type":"version","version":1}
{"type":"channel","channel":{"team":"core","name":"town-square","display_name":"Town Square","type":"O"}}
{"type":"user","user":{"username":"alice","first_name":"Alice","last_name":"Anderson"}}
{"type":"user","user":{"username":"bob","nickname":"Bobby"}}
{"type":"post","post":{"team":"core","channel":"town-square","user":"alice","message":"Standup in five","create_at":1705311000000,"reactions":[{"user":"bob","emoji_name":"thumbsup"}],"replies":[{"user":"bob","message":"On my way","create_at":1705311120000}]}}
```

Output — summary plus transcript:

```text
Bulk export format version: 1
Channels in export: 1
Users in export: 2
Messages after filters: 2

--- #town-square (Town Square) ---

[2024-01-15 09:30:00 UTC] Alice Anderson: Standup in five [reactions: :thumbsup:]
    ↳ [2024-01-15 09:32:00 UTC] Bobby: On my way
```

## Options

| Option | What it does |
| --- | --- |
| **Output** | Choose transcript only, summary only, or both. |
| **Format** | Plain text, Markdown, escaped HTML, or CSV rows for spreadsheets and downstream scripts. |
| **Channel filter** | Keep one channel by name or display name. A leading `#` is ignored. Direct messages are excluded while a channel filter is active. |
| **Author filter** | Keep messages by username or resolved display name, case-insensitive. A leading `@` is ignored. |
| **From / To date** | Inclusive UTC date bounds in `YYYY-MM-DD` form, compared against each message's timestamp. |
| **Include direct messages** | On by default; turns `direct_post` lines into "Direct message: Alice, Bob" transcript sections. |
| **Include thread replies** | On by default; replies are nested under their root post. Turn off for root posts only. |
| **Max messages** | Chronological cap after filtering. `0` means no cap; use a small number to preview a large export. |

## Limits & edge cases

- **Input shape:** paste the text contents of `import.jsonl`, not the `.zip`/`.tar` archive. Extract the archive first and open the JSONL file as text.
- **Timestamps are UTC.** Mattermost stores `create_at` as Unix milliseconds; this tool renders UTC dates and times so filtering is reproducible.
- **Unknown records are ignored.** Newer Mattermost export line types do not break the transcript. Malformed JSON, missing `type` fields and reversed date bounds produce explicit errors.
- **Attachments are referenced, not downloaded.** The transcript includes `[attachment: path]` placeholders from the export metadata. It does not unpack or fetch files.
- **No deleted/edited-message recovery.** The export only contains what Mattermost wrote into the bulk export. This tool does not query a server or reconstruct history outside that file.
- **Large exports can be heavy in the browser.** Use filters and the max-message cap to preview, then remove the cap for the final transcript.

## FAQ

<details>
<summary>How do I get the file this tool expects?</summary>

Run or download a Mattermost bulk export, extract the archive, and open the `import.jsonl` file inside it. Paste that file's text here. The tool expects JSON Lines: one JSON object per line, each with a `type` field.

</details>

<details>
<summary>Can I paste the whole export archive?</summary>

No. Archives contain files plus the JSONL metadata, and this browser tool intentionally avoids archive/file-system unpacking. Extract the archive first and paste only `import.jsonl`.

</details>

<details>
<summary>Does this include direct messages and thread replies?</summary>

Yes by default. Direct messages become their own transcript sections labelled by members, and replies are nested under their parent message. You can turn either off with the checkboxes.

</details>

<details>
<summary>How are author names resolved?</summary>

User records are indexed before rendering messages. The display label prefers nickname, then first + last name, and finally username if no friendly name is present. Filters match either username or the resolved display label.

</details>

<details>
<summary>What does CSV format contain?</summary>

Transcript CSV uses `timestamp,channel,author,username,kind,message`. Summary CSV uses `section,key,value` rows for totals plus per-channel and per-author tallies. Cells with commas, quotes or newlines are quoted.

</details>

<details>
<summary>Are my Mattermost messages uploaded?</summary>

No. The parser runs locally in WebAssembly in your browser. The page does not need an account or a Mattermost server connection, and it never sends the export contents anywhere.

</details>
