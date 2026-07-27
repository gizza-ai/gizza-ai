## About this tool

Paste text that contains emoji and pictographic symbols, then remove them in a deterministic local text pass. The converter works by Unicode grapheme cluster, so multi-codepoint emoji such as family ZWJ sequences, flags, keycaps, and skin-tone variants are removed as whole units instead of leaving invisible joiners or modifiers behind.

Choose whether each removed emoji disappears, becomes a single space, or becomes a custom placeholder such as `[emoji]`. Optional whitespace cleanup collapses the double spaces that can appear after deletion, while the text-symbol option lets you keep characters such as ©, ®, ™, and bare text hearts when you want a less aggressive cleanup.

### Worked example

Input:

```text
Ship it 🚀🔥 team 👨‍👩‍👧‍👦🇬🇧
```

With `mode=remove` and whitespace collapse enabled, the output is:

```text
Ship it team
```

With `mode=placeholder` and placeholder `[emoji]`, `Great work 👏 everyone 🎉` becomes `Great work [emoji] everyone [emoji]`.

### Limits and edge cases

- This is a text cleaner, not an image or document parser; paste the text you want to clean.
- Emoji detection follows grapheme clusters and curated Unicode ranges. It removes future reserved pictographic code points in the emoji planes, matching how Unicode reserves those blocks for emoji.
- A bare text symbol such as `❤` can be kept with the text-symbol option, but the emoji-styled form `❤️` is still removed.
- Whitespace collapse trims leading and trailing whitespace and preserves a run containing line breaks as one newline.

## FAQ

<details>
<summary>Will this remove family emoji, flags, and skin tones cleanly?</summary>

Yes. The tool scans extended grapheme clusters, so `👨‍👩‍👧‍👦`, `🇬🇧`, `👍🏽`, and `1️⃣` are each treated as a single removable unit.

</details>

<details>
<summary>What is the difference between remove, space, and placeholder?</summary>

Remove deletes the emoji completely. Space leaves one space for each emoji, which can prevent neighboring words from joining. Placeholder inserts your custom text, such as `[emoji]`, for each removed emoji.

</details>

<details>
<summary>Why would I enable collapse whitespace?</summary>

Deleting emoji can leave double spaces or leading/trailing gaps. Collapse whitespace tidies those runs into one space, preserves paragraph breaks as one newline, and trims the final result.

</details>

<details>
<summary>Can I keep symbols like ©, ®, ™, or a plain heart?</summary>

Yes. Enable the text-symbol option to keep symbols that default to text presentation. Emoji-styled variants, such as a heart followed by the emoji variation selector, are still removed.

</details>
