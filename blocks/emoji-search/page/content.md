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
