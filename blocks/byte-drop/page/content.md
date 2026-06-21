## What this tool does

**Byte Drop** removes a contiguous range of bytes from your data and returns the
remainder — instantly, right in your browser. Give it a **Start offset** and a
**Length**, and it deletes those bytes and keeps everything else. Nothing is sent
to a server: it runs locally, works offline, and needs no sign-up.

It works on raw bytes, so it can interpret your input as **text**, a **hex** byte
string, or **Base64**, and render the leftover bytes in any of those formats.

## How it works

1. The **input** is decoded into raw bytes using the **Input format** you pick.
2. Starting at the **Start offset** (a 0-based byte position), **Length** bytes
   are removed.
3. The remaining bytes — everything before the offset plus everything after the
   removed range — are joined and shown in the **Output format**.

Offsets and lengths are counted in **bytes**, not characters. For plain ASCII
text a byte equals a character, but a multi-byte character (like an emoji or an
accented letter) spans several bytes — switch the input/output to **hex** to see
exactly where the byte boundaries fall.

## Options

| Option | What it does |
| --- | --- |
| **Start offset** | The 0-based byte position of the first byte to remove. A **negative** value counts from the end (`-1` is the last byte). Clamped to the buffer. |
| **Length** | How many bytes to remove from the offset. `0` removes nothing; a range that runs past the end is clamped. Must be zero or positive. |
| **Input format** | How to read your input: **text** (UTF-8), **hex** (`48 65 6c` or `0x48656c`), or **base64**. |
| **Output format** | How to show the remainder: **text** (UTF-8), **hex**, or **base64**. Use hex or base64 for binary data. |

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hello, World` | start 5 · length 4 · text → text | `Hellorld` |
| `abcdef` | start 0 · length 3 · text → text | `def` |
| `abcdef` | start -2 · length 2 · text → text | `abcd` |
| `00112233` | start 1 · length 2 · hex → hex | `0033` |
| `SGVsbG8=` | start 0 · length 1 · base64 → text | `ello` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Are offsets in bytes or characters?** Bytes. For ASCII text the two are the
same, but a multi-byte character spans several bytes. Switch to **hex** input/output
to work at the exact byte level.

**What happens if my range goes past the end?** It's clamped — the tool removes
only the bytes that exist and never errors on an out-of-range length.

**My output says the bytes aren't valid UTF-8.** Removing part of a multi-byte
character can leave bytes that don't form valid text. Switch the **Output format**
to **hex** or **base64** to view the raw remaining bytes.

**How do I remove from the end?** Use a negative **Start offset** — for example
`-4` with length `4` drops the last four bytes.
