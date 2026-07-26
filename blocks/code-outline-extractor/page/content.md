## About this tool

Paste a source file and get a **nested outline of its functions, classes, and
methods** — the same "symbol tree" or Outline view your editor shows, but without
installing anything. It's a fast way to map an unfamiliar file, build a table of
contents for documentation, or prepare a compact structure summary before feeding
code to an LLM.

The extractor supports **Python, JavaScript, TypeScript, Rust, Go, Java, C, C++,
C#, PHP, and Swift**. Leave the language on **Auto-detect** and it guesses from the
code (Python by indentation, everything else by brace nesting), or pick a language
explicitly if the guess is wrong. Methods nest under their class, and inner
functions nest under the function that contains them.

Choose one of three output shapes:

- **Indented tree** — a plain-text outline, one entry per line, indented by depth.
- **Markdown list** — a nested bullet list you can paste straight into docs as a
  table of contents.
- **JSON** — a nested array of `{kind, name, line, signature, children}` for
  scripting or further processing.

Turn on **Show full signatures** to display each entry's whole header line (for
example `def add(a, b)`) instead of just its name, and **Show line numbers** to
annotate every entry with its source line.

Everything runs **locally in your browser** via WebAssembly — your code is never
uploaded to a server. Because the outline comes from a dependency-free
brace-depth / indentation heuristic rather than a full language parser, unusual
multi-line signatures or heavily macro-generated code may occasionally be missed;
it's built for quick navigation, not compiler-grade accuracy.

## FAQ

<details>
<summary>Which languages does it support?</summary>

Python, JavaScript, TypeScript, Rust, Go, Java, C, C++, C#, PHP, and Swift. Leave
the **Language** menu on **Auto-detect** to let the tool guess, or select one
explicitly when the code is ambiguous (for example a short snippet with no
language-specific keywords).

</details>

<details>
<summary>Is my code uploaded anywhere?</summary>

No. The whole outline is computed in your browser with WebAssembly. Nothing is
sent to a server, so it's safe to paste proprietary or unpublished code.

</details>

<details>
<summary>How does it detect functions and classes without a real parser?</summary>

It uses a deterministic heuristic instead of a full language parser (those are C
libraries that don't run in the browser sandbox). For brace languages it tracks
bracket depth — ignoring brackets inside strings, character literals, and
comments — and treats keyword declarations (`class`, `struct`, `fn`, `function`,
`def`, `impl`, …) and block-opening `name(args) {` signatures as symbols. For
Python it uses indentation. This is fast and dependency-free, but unusual
multi-line signatures may be missed.

</details>

<details>
<summary>Why is a method sometimes labelled "function"?</summary>

A `function` is relabelled `method` only when it sits directly inside a
class-like construct (`class`, `struct`, `interface`, `trait`, `impl`, `enum`). A
standalone function, or one the heuristic couldn't nest under a class, stays
labelled `function`.

</details>

<details>
<summary>What do the three output formats give me?</summary>

**Indented tree** is a plain-text outline, one symbol per line. **Markdown list**
is a nested bullet list you can drop into a README as a table of contents. **JSON**
is a nested array of `{kind, name, line, signature, children}` objects — the JSON
form always includes the signature and line fields regardless of the toggles.

</details>
