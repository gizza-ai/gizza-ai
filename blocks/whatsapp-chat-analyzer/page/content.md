## About this tool

Paste the text from an exported WhatsApp chat and get a local, deterministic report: total messages, participants, media and link counts, messages per person, busiest hours and weekdays, and the most-used words and emoji.

The parser handles the common iOS bracketed export format (`[2024-01-05, 21:07:33] Alice: ...`) and the Android dash format (`05/01/2024, 21:07 - Alice: ...`). Multi-line messages are folded into the message above them, system notices are skipped, and media placeholders such as `<Media omitted>` are counted without treating the placeholder as chat text.

Example input:

```text
[2024-01-05, 21:07:33] Alice: Hey Bob 😂😂 how are you?
[2024-01-05, 21:08:01] Bob: good thanks! www.example.com
[2024-01-06, 09:15:00] Alice: image omitted
[2024-01-06, 09:16:10] Bob: nice 😂
```

Useful limits and edge cases:

- Ambiguous numeric dates can be auto-detected, forced to day/month/year, forced to month/day/year, or parsed as year-month-day for ISO exports.
- `Top words / emoji to list` accepts `0` for every ranked entry; very large chats may produce long reports when this is set to `0`.
- Emoji counting groups skin tones, flag pairs, variation selectors, and family-style ZWJ emoji sequences as single emoji tokens.
- This is a text-export analyzer. It counts media placeholders from the export text; it does not inspect a WhatsApp `.zip` attachment bundle or read images, audio, or videos.

## FAQ

<details>
<summary>How do I get the chat text to paste here?</summary>

Use WhatsApp's chat export feature and choose the text export. Open the exported `_chat.txt` or `.txt` file, copy the contents, and paste them into the text box. The tool runs in your browser and does not upload the chat.

</details>

<details>
<summary>Which WhatsApp export formats are supported?</summary>

The analyzer supports iOS-style bracketed lines such as `[2024-01-05, 21:07:33] Alice: ...` and Android-style dash lines such as `05/01/2024, 21:07 - Alice: ...`. It also handles common AM/PM times, invisible direction marks, and continuation lines.

</details>

<details>
<summary>Why can the date format change the busiest day result?</summary>

Dates like `05/01/2024` are ambiguous: they can mean 5 January or May 1. Leave the setting on auto for most exports, or choose day/month/year or month/day/year if you know how the export was written.

</details>

<details>
<summary>Does this read images, videos, or voice notes from a WhatsApp ZIP?</summary>

No. It analyzes the text export only. Media placeholders are counted as media messages, but attachments themselves are not opened or inspected.

</details>
