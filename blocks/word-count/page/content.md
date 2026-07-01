## About this tool

The Word Counter counts words, characters, and lines in any block of text.
Paste an essay, code snippet, message, or anything else — the result is instant.

**Words** are defined as contiguous non-whitespace sequences. **Characters** are
Unicode scalar values (every letter, digit, space, emoji, or punctuation mark).
**Lines** follow Rust's `str::lines()` convention: a trailing newline does not add
an extra empty line.

Everything runs in your browser via WebAssembly — your text is never sent to a
server.

## FAQ

<details>
<summary>What exactly counts as a "word"?</summary>

Any contiguous run of non-whitespace characters. So `state-of-the-art` and
`don't` each count as one word, `3.14` is one word, and punctuation stuck to a
word (`hello,`) doesn't add extra words. This matches what `wc -w` does, and can
differ slightly from word processors that split on hyphens.

</details>

<details>
<summary>Does an emoji count as one character?</summary>

Characters are counted as Unicode scalar values. A simple emoji like 🎉 is one;
composed emoji — skin-tone modifiers, flags, or family sequences joined with
zero-width joiners — are several scalar values, so they count as more than one.
That's also why the count can differ from a tool that counts UTF-8 bytes.

</details>

<details>
<summary>Why doesn't a trailing newline add a line?</summary>

Line counting follows Rust's `str::lines()` convention: lines are the segments
separated by newlines, so `"a\nb\n"` is 2 lines, not 3. An entirely empty input
has 0 lines.

</details>

<details>
<summary>Is there a length limit, and is my text private?</summary>

There's no fixed cap — even very long documents count instantly, since the
whole computation is a single pass running locally in WebAssembly. Nothing you
paste is uploaded anywhere.

</details>
