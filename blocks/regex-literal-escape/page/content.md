## About this tool

Regex Literal Escape turns arbitrary text into a regex-safe literal, so the pattern matches the text itself instead of treating punctuation as regex syntax. It is useful when you interpolate user input into a larger pattern, build search tools, generate deny-list rules, or need to paste a literal URL/path into a regex tester.

Different engines escape different character sets, so the tool includes nine flavors:

- PCRE / PHP `preg_quote()`
- JavaScript's common `new RegExp()` helper
- JavaScript `RegExp.escape()` strict escaping
- Python `re.escape()`
- Go / RE2 `regexp.QuoteMeta()`
- .NET `Regex.Escape()`
- Java `Pattern.quote()`
- Ruby `Regexp.escape()`
- Rust `regex::escape()`

Use `delimiter` when your regex language also needs the pattern delimiter escaped, such as `/` in `/.../`. Turn on `escape_whitespace` for extended/free-spacing modes where a raw space can be ignored. Turn on `string_literal` when you want the escaped regex text double-escaped for a source-code string.

### Worked example

Input:

```text
a.b*c+(d)
```

With `flavor=pcre`, the output is:

```text
a\.b\*c\+\(d\)
```

That pattern matches the literal characters `a.b*c+(d)`, not "a, any character, b repeated, c, one or more...".

### Limits and edge cases

- Input is capped at 100,000 characters.
- Java uses `\Q...\E` quoting rather than backslash-prefixing each character.
- `javascript-strict` follows `RegExp.escape()` behavior and may emit `\xNN` for the first letter or digit and for punctuation that cannot be safely backslash-escaped.
- Alphanumeric, underscore, and whitespace delimiters are rejected because they can form ambiguous regex escapes.

## FAQ

<details>
<summary>Why do different flavors produce different output?</summary>

Regex engines do not all treat punctuation the same way. For example, Go/RE2 leaves `-` and `#` alone, while PCRE and Rust escape them. Pick the flavor that matches the engine where you will run the pattern.

</details>

<details>
<summary>Should I enable string literal escaping?</summary>

Enable it only when you are pasting the result inside quoted source code, such as a JavaScript, C#, Java, or Go string. If you are pasting directly into a regex field or a raw string, leave it off so backslashes are not doubled.

</details>

<details>
<summary>What is the delimiter field for?</summary>

Some regex syntaxes wrap patterns in a delimiter, for example `/literal/` in JavaScript or PHP. If your literal text can contain that delimiter, set `delimiter=/` so the delimiter is escaped too.

</details>

<details>
<summary>Does this validate that my whole regex is correct?</summary>

No. This tool escapes literal text. It does not parse or validate a larger regex. Use the escaped output as one safe literal fragment inside your own pattern.

</details>
