## About this tool

Chat logs come in a dozen inconsistent shapes: a WhatsApp `.txt` export uses
`[2023-01-05, 10:04] Alice: message` or `05/01/2023, 10:04 AM - Alice: message`,
an IRC or Discord copy-paste uses `<Alice> message` or `[10:04] <Alice> message`,
and a hand-typed log is often just `Alice: message`. This formatter parses all of
those line shapes and re-emits them as **one** consistently-formatted transcript,
so a mixed-source paste comes out uniform.

You control three things independently. **Speaker style** picks how each name is
rendered — plain `Name:`, Markdown-bold `**Name:**`, IRC-style `<Name>`, or an
uppercased screenplay `NAME:`. **Timestamps** can be kept verbatim, normalized to
24-hour or 12-hour clocks, or dropped entirely. And two toggles tidy the layout:
merge consecutive turns from the same speaker into one block, and add a blank line
between turns for a paragraph-style read. Dates from WhatsApp exports are dropped
by default; turn on **Keep dates** to retain them.

Everything runs locally in your browser — the transcript is parsed by a small
WebAssembly module with fixed rules, so nothing is uploaded and there is no AI
guesswork. Lines that don't match any recognized speaker or timestamp shape are
folded into the previous message as continuations, which is how wrapped or
soft-broken lines get stitched back together.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions -->

<details>
<summary>Which chat formats does it understand?</summary>

WhatsApp exports in both the bracket form (`[2023-01-05, 10:04] Name: msg`) and
the dash form (`05/01/2023, 10:04 AM - Name: msg`), bracketed or parenthesized
timestamps (`[10:04] Name:` and `(10:04 AM) <Name>`), a bare leading time
(`10:04 Name: msg`), IRC/Discord angle form (`<Name> msg`), and plain `Name: msg`
lines. You can paste a mix of these and they all normalize to the same output.

</details>

<details>
<summary>What happens to a line with no speaker or timestamp?</summary>

It is treated as a continuation and folded into the previous message. This is what
makes wrapped lines — a long message that got soft-broken across two lines, or a
pasted paragraph — reassemble into a single turn instead of becoming orphaned
lines.

</details>

<details>
<summary>Why is a plain `Word: text` line read as a speaker?</summary>

Because the tool is deterministic — it has no way to know whether `Word:` is a
name or just a sentence with a colon, so in a chat-log context it always treats a
leading `Word:` (followed by a space) as a speaker label. A bare URL like
`http://example.com` is safe: the colon there isn't followed by a space, so it's
never mistaken for a speaker.

</details>

<details>
<summary>Does anything get sent to a server?</summary>

No. The transcript is parsed entirely in your browser by a WebAssembly module.
Nothing is uploaded, and there's no LLM or network call — the same input always
produces the same output.

</details>
