## About this tool

Switch quoted text and code from one quote style to another without sending anything to a server. Paste a snippet that uses single quotes and get double quotes, go the other way, straighten curly typographic quotes into straight ones, or normalize a file that mixes all four styles onto a single delimiter.

The difference from a find-and-replace is that this tool reads the text rather than blindly substituting characters. A backslash escape is never mistaken for a delimiter, a quote that no longer needs escaping is unescaped, a quote that would now terminate the string early is escaped, and word-internal apostrophes such as `don't`, `it's` and `O'Hara` are left alone.

### Worked example

Input, with `Convert = Single → double`:

```text
print('hello, world')
greeting = 'it\'s a test'
title = 'He said "hi"'
path = 'C:\\tmp'
```

Output:

```text
print("hello, world")
greeting = "it's a test"
title = "He said \"hi\""
path = "C:\\tmp"
```

Three things happened there: `\'` lost its now-unnecessary backslash, the inner `"hi"` gained one so the new literal still closes in the right place, and `\\` — a real escaped backslash, not a quote — passed through untouched.

### Escape styles

- **Backslash** writes `\"` and `\'`. This is what C, JavaScript, TypeScript, Python, JSON, Rust, Go, Java and PHP expect.
- **Doubled** writes `""` and `''`. This is the SQL, CSV and Pascal convention — pick it when converting values destined for a database or a spreadsheet.
- **Leave bare** writes the inner quote unchanged. Correct for prose, but it produces a literal that will not parse in code.

### Limits and edge cases

- Each run accepts up to 1,000,000 bytes (1000 KB) of input.
- A quote is read as an opening delimiter only if a matching closing quote is found later in the text; an opening quote with no partner is left exactly as it was, or reported when `Quote with no partner = Report an error`.
- Escapes are read as backslash escapes on input. Doubled quotes (`''`) are an output style only, so SQL-style doubled input is not interpreted as an escaped quote.
- A single quote between two word characters is treated as an apostrophe. A trailing possessive such as `dogs'` sits at a word boundary, so it may be read as an opening delimiter; it will be left untouched unless a later quote closes it.
- Quoted runs may span multiple lines, which suits prose but means a stray quote in a source file can pair with one further down. The JSON report's `unbalanced` count is the quickest way to spot that.
- The tool converts delimiters and escapes only. It is not a language parser, so it does not know about comments, regular expressions, template literals, or triple-quoted and raw strings.

## FAQ

<details>
<summary>Will it break contractions and possessives like <code>don't</code> or <code>it's</code>?</summary>

No. With `Keep apostrophes` on — the default — a `'` or `’` sitting between two word characters is treated as an apostrophe rather than a delimiter, so `don't stop` survives inside `'don't stop'` and plain prose containing `It's Sarah's` is returned unchanged. Turn the option off if your text really does use single quotes tight against letters as delimiters.

</details>

<details>
<summary>What happens to quotes that are already escaped?</summary>

They are respected on the way in and rewritten for the new style on the way out. `"a \" b"` converted to single quotes becomes `'a " b'` — the backslash is dropped because a double quote is harmless inside a single-quoted literal. Escapes that are not quotes, such as `\n`, `\t` and `\\`, pass through unchanged.

</details>

<details>
<summary>Can it straighten curly or smart quotes?</summary>

Yes. Choose `Curly → double` or `Curly → single` to turn `“x”` and `‘x’` runs into straight delimiters, or `Any style → double` to normalize a document that mixes straight, curly, single and double quotes onto one delimiter in a single pass. Curly apostrophes inside a run, like `it’s`, are preserved.

</details>

<details>
<summary>How do I convert quotes for SQL or CSV instead of code?</summary>

Set `Escape inner quotes as` to `Doubled`. An inner quote is then written twice — `'it''s here'` — which is how SQL string literals and CSV fields escape a quote, instead of the backslash form used by most programming languages.

</details>

<details>
<summary>What does the tool do with an unmatched quote?</summary>

By default it leaves that one character exactly as it was and converts everything else, so a stray quote never causes the rest of the file to be mangled. Switch `Quote with no partner` to `Report an error` to fail instead, with the character position named, or tick `Show a JSON report with counts` to see how many runs were converted, how many inner quotes were escaped and how many lone quotes were found.

</details>
