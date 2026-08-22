## About this tool

Music File Renamer turns exported audio metadata into a rename and folder-move plan. Paste rows from a tag editor, `ffprobe`, `exiftool`, or a music-library script; choose a pattern such as `{artist}/{album}/{track} {title}`; then review every proposed `current -> new` path before doing anything on disk.

The tool is preview-only. It does not read folders, upload tracks, rewrite tags, or move files. That makes it safe for planning a cleanup, sharing a proposed library layout, or generating a shell script you can inspect before running locally.

## Input formats

CSV and TSV inputs need a header row. JSON inputs can be an array of objects, a single object, or a wrapper such as `{ "tracks": [...] }` or an `ffprobe`-style `{ "format": { "filename": "...", "tags": { ... } } }` object. Key/value inputs accept blank-line-separated blocks such as `filename=track01.mp3` and `TAG:artist=Tame Impala`.

Common field names are normalized: `file`, `filename`, `path`, and `SourceFile` all identify the current path; `album artist`, `albumartist`, `TPE2`, and `band` map to `{albumartist}`. Unknown columns are still available as tokens after normalizing their names, so an `ISRC` column can be used as `{isrc}`.

## Worked example

Input:

```csv
file,artist,album,track,title
track01.mp3,Tame Impala,Currents,1,Let It Happen
track02.mp3,Tame Impala,Currents,2,Nangs
```

Pattern:

```text
{artist}/{album}/{track} {title}
```

Output:

```text
2 tracks, 2 renames, 0 unchanged, 0 collisions, 0 skipped

track01.mp3  ->  Tame Impala/Currents/01 Let It Happen.mp3
track02.mp3  ->  Tame Impala/Currents/02 Nangs.mp3
```

## Pattern tokens and safety options

Use `{token}` to insert a tag value and `{albumartist|artist|Unknown}` to try fallbacks from left to right. Slash characters in the pattern create folders. `{track}` is normalized and zero-padded with the `track_padding` setting; `{year}` takes the first four-digit year from date-like fields.

`charset=windows` is the default because it is safe across Windows, macOS, Linux, NTFS, exFAT, and most sync tools. `charset=unix` only removes path separators and NUL-like characters. `charset=ascii` also folds common accented Latin characters before sanitizing. Collisions are detected case-insitively so a plan that would create the same destination twice is flagged before you move anything.

## Output formats

`table` is easiest to review in the browser. `list` is compact for notes. `csv` works well for spreadsheets. `json` is the machine-readable form for scripts. `sh` emits a conservative `/bin/sh` plan with `mkdir -p` and `mv -n`; review it before running and execute it from the folder that contains the current paths.

## Limits and edge cases

- The maximum batch size is 5000 records per run; 5001 records returns an error instead of trying to render an unreviewable plan.
- The tool computes paths only. It cannot inspect a music folder, fetch missing tags, fingerprint audio, or delete emptied folders.
- Missing tags follow `on_missing`: substitute `unknown_text`, skip the file, or keep its original path unchanged.
- `max_component` limits each folder or file-name component to 8-255 characters. If `keep_extension` is enabled, the extension is reattached after truncation.
- Windows reserved device names such as `CON`, `PRN`, and `LPT1` are defused when using the Windows or ASCII charset.

## FAQ

<details>
<summary>Can this rename files directly?</summary>

No. It intentionally outputs a plan only. Use the `sh` output if you want a script, review it, and run it yourself in a local shell.

</details>

<details>
<summary>How do I get the tag dump?</summary>

Export CSV/TSV from a tag editor, run `ffprobe -show_format`, run `exiftool`, or use a music metadata library to write JSON. The tool accepts all of those shapes as pasted text.

</details>

<details>
<summary>What is the difference between count, padding, and track numbers like 3/12?</summary>

`track_padding` controls how `{track}` is rendered. Values such as `3`, `03`, and `3/12` all use the first track number and can become `03` with the default two-digit padding.

</details>

<details>
<summary>Why did a file show as a collision?</summary>

Two or more records produced the same target path after token replacement, sanitizing, case handling, and extension handling. Change the pattern or include another distinguishing tag such as `{disc}` or `{filename}`.

</details>
