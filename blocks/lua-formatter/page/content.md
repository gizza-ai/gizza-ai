## About this tool

The Lua Formatter pretty-prints and re-indents a Lua script so it is easy to read,
review and diff. Paste one-line, minified or inconsistently indented Lua and it rebuilds
the source line by line: leading indentation is recomputed from Lua block structure —
`function` / `then` / `do` / `repeat` and the `(` `[` `{` bracket family open a level,
`end` / `until` and the matching closers close one, and `else` / `elseif` dedent their own
line. Commas and semicolons are normalized (no space before, one space after), runs of
blank lines collapse to a single blank, and trailing/leading blank lines are trimmed.

It is **dialect-agnostic and forgiving**: instead of parsing the script against one Lua
grammar and rejecting anything unusual, it reformats the token stream, so it works on Lua
5.1–5.4, LuaJIT and Luau. Long strings (`[[ … ]]`, `[=[ … ]=]`), line comments (`-- …`)
and long comments (`--[[ … ]]`) are preserved verbatim — their interior is never touched —
and a keyword that happens to appear inside a string (`print("end of file")`) never affects
the indentation.

### Worked example

Input:

```lua
if x then
print(1)
end
```

Output (indent 2, spaces):

```lua
if x then
  print(1)
end
```

### Options

- **Indent** — spaces of indentation per nesting level, 1 to 8 (default 2). Ignored when
  the indent character is a tab.
- **Indent character** — indent with **spaces** (the default) or a single **tab** per
  level.
- **Quote style** — `preserve` (the default, keep each short string's own quote), `double`
  (normalize to `"…"`), or `single` (normalize to `'…'`). Quotes are re-escaped as needed —
  converting `"don't"` to single quotes yields `'don\'t'`. Long strings `[[ … ]]` are never
  requoted.

### Limits

This is a **re-indenter / beautifier**, not a full pretty-printer. It never wraps long
lines at a column width, never adds or removes line breaks within a statement, and never
renames, reorders or drops anything — only leading indentation, comma spacing and blank
runs change. Because it does not parse a grammar, it will happily format invalid Lua (the
output stays invalid). Indentation is capped at one level of change per line, which keeps a
line that opens several blocks at once (`run(function()`) symmetric with the line that
closes them (`end)`).

### Privacy

Everything runs locally in your browser via WebAssembly. Your code is never uploaded to a
server.

## FAQ

<details>
<summary>Will it tell me if my Lua has a syntax error?</summary>

No — by design. It reformats the token stream without parsing the script against a
grammar, which is what lets it accept any dialect and never reject unusual code. Invalid
Lua comes out nicely indented but still invalid; catching syntax errors is your Lua
interpreter's job (`luac -p`, `lua`, etc.).

</details>

<details>
<summary>Does it work with Luau and LuaJIT?</summary>

Yes. Because it is dialect-agnostic, LuaJIT and Luau extensions (compound assignment,
type annotations, `continue`, and other syntax) pass through as ordinary tokens — they are
re-indented alongside everything else rather than rejected. The block-opening and
block-closing keywords are the same across dialects, so the indentation comes out right.

</details>

<details>
<summary>Will formatting change my strings or comments?</summary>

Long strings (`[[ … ]]` / `[=[ … ]=]`), line comments (`-- …`) and long comments
(`--[[ … ]]`) are preserved **byte-for-byte** — their interior is never re-indented or
touched. Short strings are only changed if you pick a **Quote style** other than
`preserve`, and then only the delimiter changes (with escapes fixed up); the text inside is
kept intact.

</details>

<details>
<summary>Can it minify Lua onto one line?</summary>

No — this tool is a beautifier, not a minifier. Setting the indent to 1 still keeps every
statement on its own line. Collapsing code onto a single line (and stripping comments) is
the reverse operation and would need a dedicated minifier.

</details>

<details>
<summary>Why didn't a deeply nested line indent further?</summary>

Indentation changes by at most one level per line. This keeps lines that open several
blocks at once — like `run(function()` — symmetric with the single line that closes them
(`end)`), which is how idiomatic Lua callbacks are written. Statements that genuinely nest
one block at a time (`if` inside `for` inside `function`) still indent one level each.

</details>
