## About this tool

The **String Literal Extractor** scans a block of source code and pulls out every quoted string it contains — double-quoted, single-quoted, and backtick/template literals — while skipping anything inside line (`//`, `#`) or block (`/* */`) comments. It is language-aware: pick the language and it applies that language's rules, so a `'x'` character literal in C, Java, Rust, Go, or C# is consumed but not reported, Python triple-quoted strings are captured whole, and Go raw backticks and Rust raw strings (`r"…"`, `r#"…"#`) keep their backslashes verbatim.

Use it to collect user-facing strings for translation (i18n), audit hard-coded URLs, secrets, or SQL, or turn a messy paste of code into a clean list of its literals. Everything runs locally in your browser — nothing is uploaded.

## What you can control

- **Language** — choose from Python, JavaScript, TypeScript, Java, C#, C, C++, Go, Rust, PHP, Ruby, or Shell, or leave it on **Auto-detect** to guess Python versus a generic C/JS profile.
- **Quote style** — keep every string, or only double, single, or backtick literals.
- **Output format** — a plain list of values, JSON, or CSV. JSON and CSV include each literal's source line, column, and quote style.
- **Decode escape sequences** — turn `\n`, `\t`, `\xNN`, `\uXXXX`, and friends into their real characters. Raw strings are left untouched.
- **Unique values only** — drop duplicates, keeping the first occurrence.
- **Minimum length** — skip literals shorter than a chosen number of characters (`0` keeps them all).

## Worked example

Input (JavaScript):

`const greeting = "Hello, world";` followed by `// "this comment is skipped"` and `const name = 'gizza';`

With line numbers on, the list output is:

`Hello, world  [L1]` then `gizza  [L3]`

The commented-out string never appears, and each kept value shows the line it came from.

## Limits and edge cases

- This is a deterministic quote/comment tokenizer, not a full language parser — it does not evaluate string concatenation, interpolation values, or macros.
- A string with no closing quote on its line is treated as ending at the line break (forgiving, not an error).
- Escape decoding applies only to non-raw strings; Go raw backticks and Rust raw strings keep their backslashes.
- Output is capped at 50,000 literals for very large inputs.

## FAQ

<details>
<summary>Which languages are supported?</summary>

Python, JavaScript, TypeScript, Java, C#, C, C++, Go, Rust, PHP, Ruby, and Shell/Bash. Auto-detect distinguishes Python from a generic C/JavaScript profile; pick the language explicitly for the most accurate comment and character-literal handling.

</details>

<details>
<summary>Are strings inside comments extracted?</summary>

No. Line comments (`//`, `#`) and block comments (`/* */`) are skipped for the selected language, so quoted text inside a comment is never reported as a literal.

</details>

<details>
<summary>How are character literals like `'x'` handled?</summary>

In languages where single quotes denote a character or rune literal — C, C++, Java, C#, Go, and Rust — a `'x'` is consumed so its contents cannot confuse the scanner, but it is not reported as a string. In Python, Ruby, PHP, JavaScript, and Shell, single quotes open a normal string and are extracted.

</details>

<details>
<summary>What do the JSON and CSV formats include?</summary>

Both include one row per literal with its 1-based source line, column, quote style (double, single, or backtick), and the value. CSV values are quote-escaped so commas and quotes inside a string stay in one column.

</details>

<details>
<summary>Does the code get uploaded anywhere?</summary>

No. Extraction runs entirely in your browser via WebAssembly, so your source code never leaves your machine.

</details>
