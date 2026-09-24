## About this tool

Use this generator to turn a pasted code template into a VS Code user-snippet entry. It writes the JSON object shape VS Code expects, splits the body into an array of lines, escapes JSON-special characters, and handles VS Code snippet syntax such as `$1`, `${1:name}`, `${1|red,green|}`, `$0`, `$TM_FILENAME`, and variable transforms.

The default dollar mode is `auto`: valid snippet constructs are preserved, while stray literal dollar signs are escaped so they do not become broken tabstops. Choose `literal` when the template is plain text and every dollar should be inserted literally, or `raw` when you have already written exact VS Code snippet syntax and only need JSON escaping.

### Worked example

Use this template with name `Console log`, prefix `clog, log`, and scope `javascript,typescript`:

```js
console.log($1);
```

The output is a complete snippets-file object whose key is `Console log`, whose `prefix` is an array with `clog` and `log`, and whose `body` is `console.log($1);`. If `Append $0 if missing` is enabled, the final cursor tabstop is added to the last body line.

### Where to paste the output

For language-specific snippets, open VS Code's command palette, choose Configure User Snippets, pick a language, and paste the generated entry or file object into that JSON file. For global snippets, create or edit a `.code-snippets` file and use the `scope` field to limit languages such as `javascript,typescriptreact`.

### Limits and edge cases

This tool generates one snippet at a time and caps the template at 200,000 bytes. It does not package a VS Code extension, watch a folder, or simulate how variables expand inside VS Code. It preserves valid transforms and placeholders but cannot validate every semantic detail of VS Code's editor expansion engine.

## FAQ

<details>
<summary>Why does the tool sometimes add a backslash before a dollar sign?</summary>

In VS Code snippets, `$` starts tabstops and variables. A literal shell prompt, jQuery call, or currency symbol must be escaped as `\$`. Auto mode preserves valid snippet constructs and escapes only dollars that do not look like valid VS Code snippet syntax.

</details>

<details>
<summary>Should I choose a complete snippets-file object or a single entry?</summary>

Use `snippets-file` when you want output that can stand alone as the whole JSON file. Use `entry` when you are pasting into an existing snippets file that already has surrounding braces and other snippets.

</details>

<details>
<summary>How do multiple prefixes work?</summary>

Separate prefixes with commas or new lines, such as `clog, log`. VS Code accepts either one string prefix or an array of prefixes; this tool emits a string for one trigger and an array for multiple triggers.

</details>

<details>
<summary>What is `isFileTemplate`?</summary>

It marks the snippet as a file template for VS Code's Fill File with Snippet command. Leave it off for ordinary inline code snippets, and enable it only when the snippet is meant to replace the contents of a new file.

</details>
