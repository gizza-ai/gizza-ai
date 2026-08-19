## About this tool

Indent Block Text adds spaces, tabs, or any fixed prefix to selected lines of a text block. Use it to indent code snippets, create Markdown quotes, prefix lines with comment markers, make hanging indents, or normalize copied text by outdenting or dedenting it.

The default adds four spaces to every non-blank line. Switch `style` to `custom` and set `prefix` to values like `> `, `# `, or `// ` when you need quotes, headings, or code comments. Use `mode=outdent` to remove a known indentation unit, or `mode=dedent` to remove the common leading whitespace shared by the block.

### Worked example

Input:

```text
alpha
beta
```

With `mode=indent`, `style=custom`, `prefix=> `, and `count=1`, the output is:

```text
> alpha
> beta
```

### Limits and edge cases

- `count` accepts values from 0 to 200.
- Custom prefixes may be up to 100 characters.
- A trailing newline is preserved.
- Blank lines are skipped by default to avoid trailing whitespace; turn `skip_blank_lines` off when blank lines must also carry the prefix.
- Dedent ignores blank lines while measuring common indentation.

## FAQ

<details>
<summary>How do I add a Markdown quote marker to every line?</summary>

Set `style=custom`, `prefix=> `, `count=1`, and keep `mode=indent`. The prefix is inserted exactly as written, including the space after the greater-than sign.

</details>

<details>
<summary>What is the difference between outdent and dedent?</summary>

`outdent` removes up to `count` copies of the selected unit, such as four spaces or one custom prefix. `dedent` measures the common leading whitespace across selected non-blank lines and removes that shared prefix.

</details>

<details>
<summary>Can I make a hanging indent?</summary>

Yes. Set `lines=hanging`. The first line stays unchanged and every following line receives the indent or prefix.

</details>

<details>
<summary>Will blank lines get trailing spaces?</summary>

Not by default. `skip_blank_lines=true` leaves whitespace-only lines unchanged. Turn it off if you intentionally need blank lines to carry the same prefix.

</details>
