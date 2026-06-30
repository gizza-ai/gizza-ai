## What this tool does

Paste text that's full of **smart (curly) quotes**, **em/en dashes**, the
**ellipsis glyph**, and other typographic characters, and get back clean **plain
ASCII**. It's the fix for text copied out of Word, Google Docs, Notion, or a web
page that won't paste cleanly into code, a CSV, a config file, a terminal, or a
JSON string. Everything runs locally in your browser: nothing is sent to a
server, it works offline, and there's no sign-up.

Type or paste your text and the cleaned version updates instantly.

## What gets replaced

| Typographic character | Becomes |
| --- | --- |
| `“` `”` curly double quotes, `«` `»` guillemets, `″` double prime | `"` |
| `‘` `’` curly single quotes / apostrophe, `‹` `›`, `′` prime | `'` |
| `–` en dash, `−` minus sign, `‐` `‑` `‒` hyphens | `-` |
| `—` em dash, `―` horizontal bar | `--` (or your choice) |
| `…` horizontal ellipsis | `...` |
| non-breaking, thin, and other Unicode spaces | a regular space |
| zero-width space / joiner, word joiner, the BOM | removed |

Ordinary Unicode — accented Latin like `café`, CJK like `北京`, and emoji — is
**left untouched**. This tool only straightens punctuation and whitespace; it
does not transliterate or strip accents.

## Options

| Option | What it does |
| --- | --- |
| **Em dash becomes** | How `—` and `―` are rendered: `--` (default, the Markdown convention), `-`, or ` - ` (a spaced hyphen). |
| **Normalize spaces** | On by default. Folds non-breaking, thin, and other exotic Unicode spaces to a regular space and removes zero-width characters and the byte-order mark. Turn it off to clean only quotes and dashes. |

## Examples

| Input | Output |
| --- | --- |
| `“Hello,” she said.` | `"Hello," she said.` |
| `It’s a ‘test’` | `It's a 'test'` |
| `2010–2020` | `2010-2020` |
| `wait—what` | `wait--what` |
| `Loading…` | `Loading...` |
| `5′6″` | `5'6"` |
| `non‑breaking` (with NBSP) | `non-breaking` (plain) |

## FAQ

**Is it free and private?** Yes — your text never leaves your device, and the
tool keeps working offline once the page has loaded.

**What are "smart quotes"?** They're the curly, directional quotation marks
(`“ ” ‘ ’`) that word processors substitute for the straight ASCII quotes
(`" '`). They look nice in print but break code, regexes, CSVs, and JSON.

**Why does my pasted text have invisible characters?** Copying from the web or a
PDF often brings along non-breaking spaces and zero-width characters that look
like normal spaces but aren't — they cause mysterious "string doesn't match"
bugs. Leaving **Normalize spaces** on replaces or removes them.

**Does it change accented or non-Latin letters?** No. Unlike a slug or
transliteration tool, this cleaner only touches typographic punctuation and
whitespace — `é`, `ñ`, `北京`, and emoji pass through unchanged.

**Can I keep em dashes as a single hyphen?** Yes — set **Em dash becomes** to `-`
(or ` - ` for a spaced hyphen) instead of the default `--`.
