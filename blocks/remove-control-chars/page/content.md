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

## FAQ

<details>
<summary>Exactly which characters get removed?</summary>

Everything in Unicode category **Cc**: the C0 controls U+0000–U+001F (null,
bell, backspace, escape, form feed, …) plus DEL and the C1 range U+007F–U+009F.
Tab, line feed, and carriage return are technically in that set but are kept by
default because they're meaningful whitespace — untick *Keep tabs* / *Keep
newlines* to strip them as well.

</details>

<details>
<summary>Does it remove zero-width spaces and other invisible Unicode?</summary>

No — and that's deliberate. Characters like the zero-width space (U+200B),
byte-order mark (U+FEFF), and soft hyphen are Unicode *format* (Cf) characters,
not control (Cc) characters, so this tool leaves them alone. If an invisible
character survives cleaning, it's likely one of those rather than a control
character.

</details>

<details>
<summary>Can I replace control characters with a space instead of deleting them?</summary>

Yes — put a space (or any string) in the **Replacement** field and every
removed control character is substituted with it. Leaving it empty deletes
them outright. The replacement can even be multiple characters, e.g. `?` or
`[CTRL]` if you want the removals to be visible.

</details>

<details>
<summary>Will emoji, accents, or my line endings be affected?</summary>

No. Printable Unicode — emoji, accented letters, CJK, punctuation — is never
touched, and with *Keep newlines* on (the default) both `\n` and `\r` pass
through, so Windows CRLF line endings survive intact.

</details>
