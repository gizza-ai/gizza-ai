## About this tool

Dotenv to Shell converts a `.env` file into `export`-prefixed shell statements you
can paste into a script or `eval` — and reverses shell exports back into a plain
`.env`. It reads the same syntax your app's dotenv loader does (`KEY=VALUE` lines,
`#` comments, blank lines, single- and double-quoted values, inline comments, and
an optional `export ` prefix) and re-emits each variable with **shell-safe
quoting**, so special characters survive being sourced.

The point is the quoting. A naive `export KEY=$value` breaks the moment a value
contains a space, a `#`, a `$`, a backtick, or a quote. This tool single-quotes
every value that needs it and escapes embedded single quotes the POSIX way
(`'\''`), so `$`, backticks and `#` stay **literal** instead of being expanded or
truncated by the shell.

**Example — `.env` → shell (posix, auto quoting):**

Input:

```
# database
export DB_HOST=localhost
DB_PORT=5432
GREETING=hello world
API_TOKEN=s3cr3t$val
```

Output:

```
# database
export DB_HOST=localhost
export DB_PORT=5432
export GREETING='hello world'
export API_TOKEN='s3cr3t$val'
```

Clean values (`localhost`, `5432`) stay unquoted; `hello world` and `s3cr3t$val`
are single-quoted so the space and the `$` are preserved exactly.

Options:

- **Direction** — `.env → shell` (default) or `shell → .env` to reverse. The
  reverse parser understands `export KEY=…`, bare `KEY=…`, fish `set -gx KEY …`,
  and csh `setenv KEY …`, and rebuilds a normal `.env`.
- **Shell dialect** — `posix`/`bash` emit `export KEY=value` (also fine for
  zsh/sh); `fish` emits `set -gx KEY value` with fish's own quoting rules.
- **Quoting** — `auto` leaves safe values bare and quotes the rest; `single`
  always single-quotes for a uniform, copy-safe result.

Full-line comments and blank lines are preserved so your file's structure carries
over. Everything runs locally in your browser via WebAssembly — your `.env` is
never uploaded.

## FAQ

<details>
<summary>Is my .env file uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly, so your file
never leaves your machine — it's safe to paste real secrets. Copy the result out
when you're done; nothing is stored or sent.

</details>

<details>
<summary>Why single quotes instead of double quotes?</summary>

Single quotes make the shell treat the value **literally** — `$`, backticks and
`\` are not interpreted, so a token like `s3cr3t$val` or `p@ss w#rd` survives
intact. Double quotes would let the shell expand `$var` and run `` `command` ``,
which silently corrupts secrets. Embedded single quotes are handled with the
standard `'\''` splice, so even a value like `it's fine` round-trips correctly.

</details>

<details>
<summary>What does "auto" quoting do versus "single"?</summary>

**Auto** (the default) leaves a value unquoted when it's made only of safe
characters (letters, digits, and `_@%+=:,./-`) and single-quotes anything else —
so the output stays readable. **Single** always wraps every value in single
quotes, which is handy when you want a uniform, unambiguous result. Either way,
special characters are always kept literal.

</details>

<details>
<summary>Can it convert shell exports back into a .env file?</summary>

Yes — set **Direction** to `shell → .env`. It parses `export KEY=value`, bare
`KEY=value`, fish `set -gx KEY value`, and csh `setenv KEY value` statements,
un-quotes each value (including the POSIX `'\''` splice), and writes a plain
`.env` file, double-quoting values only when they contain spaces or special
characters.

</details>

<details>
<summary>How does fish output differ?</summary>

fish doesn't use `export`; it uses `set -gx NAME value` for a global exported
variable. Choosing the **fish** dialect emits that form and applies fish's own
single-quote escaping (only `\` and `'` are escaped), which differs from POSIX.
Pick fish if you'll `source` the result in a `config.fish` or fish script.

</details>

<details>
<summary>What happens to comments, blank lines and invalid names?</summary>

Full-line `#` comments and blank lines are passed through unchanged (both `.env`
and shell use `#`), and an inline `# comment` after an unquoted value is stripped.
A key that isn't a valid shell variable name (e.g. starting with a digit, or
containing a dash) can't be `export`ed, so it's replaced with a
`# skipped "…": not a valid shell variable name` note instead of emitting broken
syntax.

</details>

<details>
<summary>Is csh/tcsh supported?</summary>

Only as **input** for the reverse direction — the `shell → .env` parser reads
`setenv KEY value`. csh is not offered as an output dialect: its single quotes
can't contain newlines and `!` still triggers history expansion even when quoted,
so byte-safe csh output can't be guaranteed. Use posix/bash or fish for output.

</details>
