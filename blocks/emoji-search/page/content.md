## About this tool

Emoji Search finds the right emoji fast. Type a **keyword** (`happy`, `love`,
`fire`, `rocket`), a **`:shortcode:`** (`:smile:`, `:thumbsup:`), or a **category
name** (`smileys`, `people`, `animals`, `food`, `activities`, `symbols`,
`flags`) and it returns the matching glyphs — ranked best-match first — ready to
copy.

### How it works

Every query is matched, case-insensitively, against each emoji's name,
shortcode, keywords, and category. Surrounding colons in a `:shortcode:` are
ignored, so `:rocket:` and `rocket` find the same emoji. Searching a category
name lists that whole category.

- **Max results** caps how many emoji come back (1–100, default 20).
- **Glyphs only** returns just the bare emoji, space-separated, instead of the
  labelled `glyph :shortcode: name (category)` lines — handy for pasting a row
  of emoji straight into a message.

### Private and offline

The entire emoji set is bundled into the page as WebAssembly. Nothing you type
leaves your browser — there is no server call, no tracking, and no sign-up. It
works offline once the page has loaded.

## FAQ

<details>
<summary>How are the results ordered?</summary>

Best match first. An **exact** hit on the name, shortcode or a search keyword
scores highest, a category-name query lists that whole category next, and
prefix matches beat plain substring matches. So `:smile:` puts 😄 at the top
even though many emoji mention "smile" somewhere.

</details>

<details>
<summary>How many results can I get at once?</summary>

The **Max results** field accepts 1–100 and defaults to 20. Anything above 100
is clamped down to 100, and leaving it at 0/blank falls back to the default —
searching a big category like `smileys` simply returns the first N by rank.

</details>

<details>
<summary>How do I copy a row of plain emoji without the labels?</summary>

Tick **Glyphs only**. Instead of one `glyph :shortcode: name (category)` line
per match you get just the bare glyphs separated by spaces — ready to paste
into a chat message, commit message or bio.

</details>

<details>
<summary>Why can't I find a very new emoji?</summary>

The tool searches a curated dataset bundled into the page, focused on the
emoji people actually search for. Very recent Unicode additions may not be in
it yet — if a shortcode you use is missing, a keyword from the emoji's name
usually still finds a close match.

</details>
