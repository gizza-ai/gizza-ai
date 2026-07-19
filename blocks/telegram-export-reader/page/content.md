## About this tool

Telegram Export Reader turns a Telegram Desktop machine-readable `result.json`
export into plain text you can actually read: a dated transcript plus per-sender
message counts, word counts, top words, top emoji and media/service-message
counts. Paste the JSON export; everything runs locally in WebAssembly and no chat
history is uploaded.

It accepts the common Telegram Desktop shapes:

- a single-chat export with a top-level `messages` array;
- a full account export with `chats.list[].messages`;
- a bare array of message objects.

Formatted Telegram text arrays are flattened into normal text, while media-only
messages become readable placeholders such as `[photo]`, `[sticker 🎉]`, `[voice
message]` or `[file: invoice.pdf]`.

## Worked example

Input:

```json
{
  "name": "Weekend Trip",
  "type": "private_group",
  "messages": [
    {"type":"message","date":"2021-03-27T14:45:00","from":"Alice","text":"Hey everyone ready for the trip"},
    {"type":"message","date":"2021-03-28T09:46:10","from":"Bob","text":["Yes ",{"type":"bold","text":"so"}," excited 🎉"]},
    {"type":"message","date":"2021-03-28T09:47:00","from":"Bob","text":"","photo":"photos/photo_1.jpg"}
  ]
}
```

Output (`both` mode):

```text
Chat: Weekend Trip (private group)
Messages: 3
Participants: 2
Words: 9
Media messages: 1
Date range: 2021-03-27 to 2021-03-28

Messages per sender:
      2   66.67%  Bob    (3 words)
      1   33.33%  Alice  (6 words)

Top words (min length 3):
      1  everyone
      1  excited
      1  hey
      1  ready
      1  the
      1  trip
      1  yes

Top emoji:
      1  🎉

=== Transcript ===

[2021-03-27 14:45:00] Alice: Hey everyone ready for the trip
[2021-03-28 09:46:10] Bob: Yes so excited 🎉
[2021-03-28 09:47:00] Bob: [photo]
```

## Options and limits

- **Output**: `transcript`, `stats` or `both`.
- **Include service messages** adds group-created, member-invited, title-changed
  and similar system lines to the transcript and service count.
- **Sender filter** keeps only one display name, case-insensitively.
- **Max messages** caps the rendered export after filters; use `0` for all.
- The tool reads JSON only. Telegram's HTML export is not supported; export with
  **Format: Machine-readable JSON** in Telegram Desktop.
- Media files are not embedded in `result.json`, so the tool reports placeholders
  instead of extracting photos, stickers or voice notes.

## FAQ

<details>
<summary>How do I get result.json from Telegram?</summary>

In Telegram Desktop, open the chat menu, choose **Export chat history**, and set
**Format** to **Machine-readable JSON**. Telegram writes a folder containing
`result.json`; open that file in a text editor, copy all of it, and paste it
here. For a full account export, use Settings → Advanced → Export Telegram data.

</details>

<details>
<summary>Does this upload my private messages?</summary>

No. The parser is pure WebAssembly running in the browser tab. Your pasted JSON
is not sent to a server, and the output is computed locally.

</details>

<details>
<summary>Why do I see [photo] or [sticker] instead of the media?</summary>

Telegram's `result.json` references exported files by path; it does not contain
the image, sticker, audio or document bytes. This tool reads the JSON only, so it
keeps a clear placeholder in the transcript instead of trying to load files that
were not pasted.

</details>

<details>
<summary>Can I analyze just one person?</summary>

Yes. Put their Telegram display name in **Sender filter**. Matching is
case-insensitive and exact after trimming, so `Bob` matches `bob` but not
`Bobby`. The filter applies before the message cap.

</details>

<details>
<summary>What does the word count include?</summary>

Words are split on whitespace for per-sender totals, while the "Top words" list
uses alphanumeric tokens of at least three characters. Emoji are counted
separately in the "Top emoji" list.

</details>
