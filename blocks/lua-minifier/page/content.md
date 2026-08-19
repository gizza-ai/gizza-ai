## About this tool

Lua is flexible about whitespace, which makes it a good fit for minification: comments, blank lines
and indentation can usually disappear without changing how the chunk runs. This tool scans Lua as
Lua tokens instead of doing a blind regular-expression replace, so quoted strings, long strings,
long-bracket levels, shebangs and tricky token boundaries survive.

Use it when you need a smaller plugin, game script, embedded config chunk or LuaJIT helper. The
default output strips comments and joins the script onto one line. Turn on local renaming when you
want a smaller payload and the script does not inspect its own local names with debug APIs.

### Worked example

Input:

```lua
--! @license MIT
local function greet(name)
  -- build a greeting
  local message = "hello, " .. name
  print(message)
end

greet("Ada")
```

With **Strip Lua comments** and **Preserve license / copyright banners** enabled, the output keeps
the banner and removes the internal comment and spacing:

```lua
--! @license MIT
local function greet(name)local message="hello, "..name print(message)end greet("Ada")
```

If **Rename locals and parameters to short aliases** is enabled, local variables and parameters are
shortened while globals, fields, method names and string contents are left alone.

### Limits and edge cases

- The transform is token-aware, not a full Lua AST optimizer. It does not fold constants, reorder
  code, remove dead branches or encode strings.
- Local renaming is opt-in and refuses when block structure does not balance. Minification without
  renaming is still forgiving for incomplete snippets.
- `--!`, `@license`, `@preserve` and `@copyright` comments are kept by default; disable the banner
  option to strip them too.
- `line_breaks = keep` keeps one output line per non-empty source line, useful when runtime error
  line numbers matter more than the smallest possible result.
- Code that reflects over local names with `debug.getlocal`, builds code strings that mention local
  names, or depends on exact source text should not use local renaming.

## FAQ

<details>
<summary>Is this an obfuscator?</summary>

No. It is a size-reducing minifier. It removes comments and unnecessary whitespace, and it can
rename locals to shorter names, but it does not encode strings, insert control-flow traps or try to
hide what the code does. The output is still Lua source intended to run the same way as the input.

</details>

<details>
<summary>What names are safe to rename?</summary>

Only locals, `local function` names and function parameters are candidates. Globals, table fields,
method names, table-constructor keys, `goto` labels and anything inside a string are never renamed.
Aliases are unique across the file and avoid global names already used by the script.

</details>

<details>
<summary>Why preserve license comments by default?</summary>

Minification often runs right before publishing a file, and it is easy to strip required attribution
accidentally. The default keeps comments marked with `--!`, `@license`, `@preserve` or `@copyright`.
If you own the file and want the smallest possible output, turn that option off.

</details>

<details>
<summary>Will it change strings or long brackets?</summary>

No. Short strings, long strings and long-bracket contents are emitted exactly as scanned. A comment
marker such as `--` inside a string remains part of the string, and nested long-bracket levels such
as `[=[ ... ]=]` keep their original delimiters and contents.

</details>
