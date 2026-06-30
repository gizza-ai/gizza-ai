## About this tool

**Remove control characters** strips invisible, non-printable control characters
from text. These are the characters in Unicode category *Cc* — the C0 range
(U+0000–U+001F, which includes the **null byte** U+0000, the bell, backspace, and
form feed) and the C1/DEL range (U+007F–U+009F). They often sneak in when copying
from logs, binary files, terminals, or badly-encoded exports, and can break
imports, databases, and search.

By default the tool keeps the control characters you usually *want*:

- **Keep tabs** (default on): preserves horizontal tab characters (`\t`).
- **Keep newlines** (default on): preserves line feed (`\n`) and carriage
  return (`\r`) so your line structure is left intact.

Turn either off to strip those too. The **Replacement** field lets you substitute
a character (for example a single space) for every removed control character;
leave it empty to simply delete them.

### Privacy

Everything runs **in your browser** via WebAssembly — your text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat.

### Common uses

- Remove null bytes (`\0`) and other junk from text copied out of a binary file.
- Clean log or terminal output that contains escape/bell/backspace characters.
- Sanitize input before importing into a database, spreadsheet, or CSV.
- Strip invisible characters that break search, diffing, or string comparisons.
