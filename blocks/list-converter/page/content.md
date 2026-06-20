## Reformat a list in your browser

Paste a list and convert it between layouts — comma-separated, one-per-line,
bulleted, numbered, quoted, or space-separated — with optional sort and dedupe.
Everything runs locally in your browser; nothing is uploaded.

### Options

- **Input separator** — `auto` (default) splits on newlines if present, else
  commas, else semicolons. Force `comma`/`newline`/`semicolon`/`space`.
- **Output format**
  - `comma` → `a, b, c`
  - `newline` → one item per line
  - `bulleted` → `- a`
  - `numbered` → `1. a`
  - `quoted` → `"a", "b"` (handy for code arrays; quotes are escaped)
  - `space` → space-separated
- **Sort alphabetically** (case-insensitive) and **Remove duplicates** (keeps the
  first occurrence).

Items are trimmed and blank entries are dropped automatically.
