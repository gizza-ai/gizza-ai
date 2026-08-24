## About this tool

Commenting out a block of code is a one-keystroke job inside an editor and a fiddly one everywhere else. Code pasted into a chat window, a ticket, a code review, a snippet manager, or a docs draft has left the editor behind, so the markers have to go on — or come off — by hand, one line at a time, in whichever syntax that language happens to use.

This tool does what `Ctrl+/` does. Paste a block, pick a language (or leave **Auto-detect** on), and every line gets the right marker: `//` for JavaScript, TypeScript, Java, C#, C, C++, Go, Rust, Swift, Kotlin, Scala, and PHP; `#` for Python, Ruby, Perl, shell, PowerShell, YAML, TOML, R, Dockerfiles, and Makefiles; `--` for SQL, Lua, and Haskell; `;` for INI; `;;` for Clojure; `%` for LaTeX; `'` for Visual Basic; and `REM` for batch files. CSS, HTML, and XML have no line comment at all, so each line is wrapped in `/* */` or `<!-- -->` instead.

**Toggle** is the default and follows the editor rule: if every line under consideration already carries the marker, the block is uncommented; otherwise the whole block is commented. A partly commented block therefore becomes fully commented rather than half-inverted, which is what makes the operation reversible — run it twice and you are back where you started. **Comment every line** and **Uncomment every line** force a direction when you want one.

With the defaults, this:

```
  if ready:
      send(payload)
  log("done")
```

becomes this:

```
  # if ready:
  #     send(payload)
  # log("done")
```

The marker lands at the block's shallowest indentation, so relative indentation survives and the code still reads as code. Choose **Flush left at column 0** if you prefer every marker pinned to the left margin, which is the other convention editors offer.

Three smaller controls cover the rest. **Space after the marker** writes `// code` rather than `//code` and is on by default, because that is what most linters and formatters expect; uncommenting removes at most one such space, so deliberate indentation inside a comment survives a round trip. **Also comment blank lines** marks blank and whitespace-only lines too — off by default, matching editor behavior — and when on, a blank line becomes the bare marker with no trailing space. **Custom marker** overrides the language's own syntax for anything the list does not cover, such as an `@` directive or a template language; a custom marker is always treated as a line comment, never as a pair.

Limits and edge cases:

- This is a lexical transform, not a parse. A `//` inside a string literal or a URL looks like a comment marker to the tool, so a line such as `url = "https://example.com"` will be treated as already commented if you point the JavaScript profile at it.
- **Auto-detect** is a heuristic over shebangs, existing comment markers, and distinctive keywords. It is deliberately conservative and falls back to `#` for a plain list of lines. Name the language when it guesses wrong.
- Uncommenting only removes the marker that the selected language (or custom marker) defines. Mixed syntax in one block needs one pass per syntax.
- Line endings are preserved, including CRLF, and the trailing newline of your input is left as you pasted it.
- Input is capped at 2,000,000 characters.
- Nothing is uploaded. The page runs the Rust/WASM transform in your browser.

## FAQ

<details>
<summary>What exactly does Toggle do with a block that is only partly commented?</summary>

It comments the whole block. The rule is: uncomment only when *every* considered line already carries the marker; otherwise comment everything. That is the same rule editors use, and it is what makes the operation reversible — the already-commented lines pick up a second marker, and the next toggle removes exactly one layer, restoring the original mix. If you want the opposite behavior, choose **Uncomment every line**, which strips a marker where it finds one and leaves other lines untouched.

</details>

<details>
<summary>Why did Auto-detect pick the wrong language?</summary>

Auto-detect reads a shebang first, then any comment marker already present, then distinctive keywords. Short snippets often have none of those — three lines of `key: value` could be YAML, a config file, or prose — so it falls back to `#`, the least surprising choice for plain text. Pick the language from the dropdown whenever the snippet is small or the syntax is ambiguous; the language you choose always wins over detection.

</details>

<details>
<summary>How are CSS, HTML, and XML handled when they have no line comment?</summary>

Each line is wrapped in the language's block pair — `/* */` for CSS and `<!-- -->` for HTML and XML — so the result stays line-by-line and reversible. Toggling back removes the pair from every line that has one. Note that HTML and XML comments cannot legally nest, so wrapping a line that already contains `-->` produces markup a strict parser will reject; that case is worth a manual look.

</details>

<details>
<summary>Does uncommenting damage indentation inside the comment?</summary>

No. Uncommenting removes the marker and at most one padding space after it, so a comment written as `#     deep` returns `    deep` with its four spaces intact. Leading indentation before the marker is always preserved as-is, whichever marker position you used when commenting.

</details>

<details>
<summary>Is my code uploaded anywhere?</summary>

No. The page compiles the transform to WebAssembly and runs it locally in your browser, and the command-line version runs locally too. Nothing you paste is sent to a server, logged, or stored.

</details>
