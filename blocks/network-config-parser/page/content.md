## About this tool

The **network config parser** turns a hierarchical device configuration into a
structured, foldable tree you can read and search. Paste a running config from a
Cisco IOS / IOS-XE / NX-OS switch or router, an Arista EOS device, or a Juniper
Junos box, and the parser reconstructs the parent/child hierarchy — interfaces
and their sub-commands, routing processes and their networks, Junos `{ … }`
blocks and their statements.

It handles two syntaxes. **Indentation style** (Cisco, NX-OS, Arista) uses
leading whitespace to express nesting, with `!` lines as section separators.
**Brace style** (Juniper Junos) uses `{ … }` blocks with statements ending in
`;`. Leave the syntax on **auto** and the tool detects which one you pasted.

Choose how you want the result:

- **Tree** — a nested JSON array of `{ line, children }` nodes that mirrors the
  config hierarchy, ideal for feeding another script.
- **Paths** — a flat list of full command paths, one per leaf statement, with
  each ancestor joined by ` / ` (similar to Junos `set`-style lines).
- **Report** — a compact summary: the list of top-level sections plus stats
  (section count, total lines, leaf statements, maximum depth, comment count).

Type a substring into **filter** to keep only the sections and lines that match:
matching a section header keeps its whole block, and matching a deep value keeps
its ancestor chain so you never lose context. Everything runs locally in your
browser — the configuration you paste never leaves your machine.

## FAQ

<details>
<summary>Which vendors and config formats does it understand?</summary>

Two syntactic families. **Indentation style** covers Cisco IOS, IOS-XE and
NX-OS, plus Arista EOS — devices that express nesting with leading whitespace and
use `!` as a separator. **Brace style** covers Juniper Junos, which nests with
`{ … }` blocks and ends statements with `;`. The parser works on the *structure*
of the text, so any config that follows one of those two conventions parses,
regardless of the exact platform.

</details>

<details>
<summary>Is my configuration uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs entirely inside your browser
tab. The configuration you paste is never sent to a server, logged, or stored —
close the tab and it is gone. That makes it safe to use on production configs
that contain hostnames, addresses, and other sensitive details.

</details>

<details>
<summary>What is the difference between the tree, paths, and report outputs?</summary>

**Tree** gives you the full hierarchy as nested JSON (`{ line, children }`), best
for programmatic use. **Paths** flattens the tree into one line per leaf
statement, joining each command to its ancestors with ` / ` — handy for grepping
or diffing. **Report** skips the detail and returns a section list plus summary
stats (sections, total lines, leaf statements, depth, comments) so you can size
up a config at a glance.

</details>

<details>
<summary>How does the filter keep context?</summary>

The filter is a case-insensitive substring match. If the substring matches a
**section header**, the entire block under that header is kept. If it matches a
**deep value**, only that line survives, but its full chain of parent sections is
retained so the result is still a valid, readable tree. An empty filter (the
default) returns the whole config.

</details>

<details>
<summary>What happens to comments and separators?</summary>

By default (`comments = strip`) comment and separator lines are removed: bare `!`
separators, `!`- and `#`-prefixed comment lines in indentation style, and `#`-to
-end-of-line and `/* … */` block comments in brace style. Switch to
`comments = keep` to retain the prefixed comment lines as nodes in the tree; bare
`!` separators are always dropped because they carry no content. The report's
`comments` stat always counts how many were seen.

</details>
