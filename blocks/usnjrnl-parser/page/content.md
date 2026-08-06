## About this tool

The NTFS change journal lives in the `$J` alternate data stream of `\$Extend\$UsnJrnl`. Windows appends one fixed-layout `USN_RECORD` every time a file or directory changes, so `$J` ends up being a near-complete log of what was created, written, renamed and deleted on a volume — including files that no longer exist anywhere else. This tool decodes that stream in your browser: paste the bytes as hex or Base64 and read the events back as a timeline.

`USN_RECORD` V2 (64-bit file references) and V3 (128-bit `FILE_ID_128` references) are both decoded in full. V4 range-tracking records are counted and reported separately, never dressed up as file events — they carry extents, not a name or a timestamp. Because `$J` is a sparse file whose deallocated head reads back as zeroes, and because carved copies are usually ragged at both ends, the scanner validates every candidate header and resynchronises on the 8-byte record alignment NTFS guarantees. It then tells you how many bytes it skipped as sparse and how many it could not parse, so you always know how much of the input it actually used.

Every row carries the UTC timestamp, the update sequence number, the file name, the decoded `USN_REASON_*` flags, the decoded `FILE_ATTRIBUTE_*` flags, the MFT entry and sequence numbers of both the file and its parent directory, the `USN_SOURCE_*` flags and the security id. Rename records arrive in halves — one `RENAME_OLD_NAME`, one `RENAME_NEW_NAME` — and are merged into a single `old -> new` row by default; turn that off to see the raw records exactly as NTFS wrote them.

### Worked example

A four-record journal fragment: a file created, a file renamed, a file deleted. Paste this into the **Journal bytes** box with the default hex encoding and pick the **List** output format (or click the "Create, rename and delete timeline" example above):

```
50000000020000002a00000000000100050000000000010000100000000000000060ba17bf9bda0100
01000000000000000100002000000012003c006e006f007400650073002e00740078007400000050000000
020000004d000000000002000500000000000100581000000000000000e74d1bbf9bda01001000000000
0000000100002000000012003c00640072006100660074002e00740078007400000050000000020000004d
000000000002000500000000000100b01000000000000000e74d1bbf9bda01002000800000000000010000
2000000012003c00660069006e0061006c002e00740078007400000050000000020000006300000000
0003000500000000000100081100000000000000 6ee11ebf9bda010002008000000000000100002000
000014003c007300650063007200650074002e0074006d007000
```

The output is:

```
$UsnJrnl:$J — 4 records parsed, 3 matched, 3 shown
  note: 1 rename pair merged into a single Rename row (pair_renames=true).
2024-05-01T12:00:00Z  usn=4096  File create  notes.txt  [FILE_CREATE]
2024-05-01T12:00:06Z  usn=4272  Rename  draft.txt -> final.txt  [RENAME_OLD_NAME | RENAME_NEW_NAME | CLOSE]
2024-05-01T12:00:12Z  usn=4360  File delete  secret.tmp  [FILE_DELETE | CLOSE]
```

Four records went in and three rows came out, because the two halves of the `draft.txt` → `final.txt` rename were merged. Switching the output format to **Summary** on the same input reports the USN range `4096 .. 4360`, the UTC span `2024-05-01T12:00:00Z .. 2024-05-01T12:00:12Z`, three distinct MFT entries and one record in each change class.

Line breaks, spaces, colons, dashes and a leading `0x` are all ignored in hex input, so `xxd`, `xxd -p`, PowerShell and hex-editor output can be pasted as-is.

### Limits and edge cases

- One pasted journal per run, up to 48 MB of decoded bytes. A larger `$J` should be split (for example with `dd`) and parsed in chunks — the scanner resynchronises, so a chunk does not have to start on a record boundary.
- **Full paths cannot be reconstructed from `$J` alone.** A record stores the parent's *reference number*, never the parent's name. Rather than invent a path, every row emits the parent MFT entry and sequence so it can be joined against an `$MFT` listing.
- Timestamps are always rendered as UTC. A browser tool cannot know the acquisition host's timezone, and guessing it would silently mis-time a timeline.
- The journal is a circular buffer of bounded size, so old records are overwritten. An event's absence proves nothing; its presence is the evidence.
- `USN_RECORD` V4 records are range-tracking entries with no name and no timestamp. They are counted and reported, not listed as file events.
- Reason and attribute bits that are not in the documented tables are surfaced as `UNKNOWN_0x…` rather than dropped.
- Bodyfile and TLN rows are pipe-delimited with no escaping rule, so a literal `|` inside a file name is replaced with `/` to keep the row parseable.
- The record list is capped by **Maximum records** (200 by default, 5000 maximum). Summary mode always counts every matched record regardless of the cap.

## FAQ

<details>
<summary>How do I get the $UsnJrnl:$J bytes out of a volume?</summary>

Extract the `$J` alternate data stream of `\$Extend\$UsnJrnl` with a forensic acquisition tool or a raw-NTFS reader, then encode the resulting file as hex or Base64 (`xxd -p -c 256 J` or `base64 J`) and paste it here. Browser and chat tools cannot open local disk paths or read raw devices, so the encoding step is unavoidable.

</details>

<details>
<summary>Why does a deleted file still appear in the journal?</summary>

That is the point of the artifact. NTFS writes a `FILE_DELETE` record when a file is removed, and that record survives in `$J` long after the file's data and its `$MFT` entry are gone. It is one of the few places that shows a file existed, when it was created and when it was destroyed. The journal is a fixed-size circular buffer, though, so the window is finite — typically hours to weeks depending on volume size and activity.

</details>

<details>
<summary>Why is there no full path on each row?</summary>

A `USN_RECORD` stores the parent directory's file reference number, not its name. Reconstructing `C:\Users\...\notes.txt` therefore needs a second artifact — the `$MFT` — to resolve that reference chain. This tool emits the parent MFT entry and sequence number on every row so you can join against an `$MFT` listing instead of trusting an invented path.

</details>

<details>
<summary>What does merging rename pairs actually do?</summary>

NTFS records a rename as two records: `RENAME_OLD_NAME` carrying the old name, then `RENAME_NEW_NAME` carrying the new one. With merging on (the default), the two are combined into one row shown as `old.txt -> new.txt`, using the completing record's timestamp and USN. Turn it off to see the raw journal exactly as written — useful when you are validating the parse itself, or when a rename's two halves were separated by journal wrap.

</details>

<details>
<summary>What are the CLOSE records for?</summary>

Windows sets `USN_REASON_CLOSE` on the last record of a change burst, so a single file write can produce several records ending in one that carries `CLOSE`. Filtering the change class to **Close** collapses noisy multi-record bursts down to one row per completed operation, which is often the cleanest starting view for a busy volume.

</details>

<details>
<summary>Which output format should I use?</summary>

**Summary** for triage — counts, USN range, UTC time span and the most-active names across everything that matched. **Report** or **List** for reading. **CSV** for a spreadsheet or a pandas dataframe. **Bodyfile** to feed Sleuth Kit `mactime`. **TLN** for pipe-delimited `epoch|source|host|user|description` timeline rows (set the TLN host name field). **JSON** when you want every decoded field plus the scan accounting for further processing.

</details>
