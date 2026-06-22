## What this tool does

**Byte Take** extracts a contiguous slice of bytes from your data and returns
just that slice — instantly, right in your browser. Give it a **Start offset**
and a **Length**, and it keeps those bytes and drops everything else. Nothing is
sent to a server: it runs locally, works offline, and needs no sign-up.

It works on raw bytes, so it can interpret your input as **text**, a **hex** byte
string, or **Base64**, and render the extracted bytes in any of those formats.

## How it works

1. The **input** is decoded into raw bytes using the **Input format** you pick.
2. Starting at the **Start offset** (a 0-based byte position), **Length** bytes
   are taken.
3. The extracted bytes are shown in the **Output format**.

Offsets and lengths are counted in **bytes**, not characters. For plain ASCII
text a byte equals a character, but a multi-byte character (like an emoji or an
accented letter) spans several bytes — switch the input/output to **hex** to see
exactly where the byte boundaries fall.

## Options

| Option | What it does |
| --- | --- |
| **Start offset** | The 0-based byte position of the first byte to extract. A **negative** value counts from the end (`-1` is the last byte). Clamped to the buffer. |
| **Length** | How many bytes to extract from the offset. `0` extracts nothing; a range that runs past the end is clamped. Must be zero or positive. |
| **Input format** | How to read your input: **text** (UTF-8), **hex** (`48 65 6c` or `0x48656c`), or **base64**. |
| **Output format** | How to show the slice: **text** (UTF-8), **hex**, or **base64**. Use hex or base64 for binary data. |

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hello, World` | start 5 · length 4 · text → text | `, Wo` |
| `abcdef` | start 0 · length 3 · text → text | `abc` |
| `abcdef` | start -2 · length 2 · text → text | `ef` |
| `00112233` | start 1 · length 2 · hex → hex | `1122` |
| `SGVsbG8=` | start 1 · length 3 · base64 → text | `ell` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Are offsets in bytes or characters?** Bytes. For ASCII text the two are the
same, but a multi-byte character spans several bytes. Switch to **hex** input/output
to work at the exact byte level.

**What happens if my range goes past the end?** It's clamped — the tool extracts
only the bytes that exist and never errors on an out-of-range length.

**My output says the bytes aren't valid UTF-8.** Slicing through the middle of a
multi-byte character can leave bytes that don't form valid text. Switch the
**Output format** to **hex** or **base64** to view the raw extracted bytes.

**How do I grab the last few bytes?** Use a negative **Start offset** — for
example `-4` with length `4` takes the last four bytes.

**How is this different from byte drop?** Byte Take keeps the selected range and
discards the rest; Byte Drop does the opposite — it removes the selected range
and keeps the rest.
