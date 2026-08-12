## About this tool

`strip-console-logs` deletes the `console.*` debug statements you accumulated while building a
feature, so the source you ship no longer prints to the browser console. Paste JavaScript,
TypeScript, JSX or TSX, choose which console methods to strip, and copy the cleaned source back
out. Everything runs locally in your browser — the code you paste never leaves the page.

The scanner is token-aware rather than a regular expression. It walks the source tracking string
literals, template literals, regular expressions and comments, so text that merely *looks* like a
call — `const s = "console.log(1)"`, `// console.log(1)`, `/console\.log\(/` — is left exactly as
it was. Calls that span several lines, or that nest parentheses and strings inside their
arguments, are matched as a whole.

### Worked example

Input:

```js
function checkout(cart) {
  console.log("cart", cart);
  let total = cart.reduce((a, i) => a + i.price, 0);
  console.debug(`total ${total}`);
  if (total > 100) {
    console.info("free shipping");
    total -= 5;
  }
  console.error("payment failed");
  return total;
}
```

With the defaults (`methods = log,debug,info,warn`, `action = remove`) the output is:

```js
function checkout(cart) {
  let total = cart.reduce((a, i) => a + i.price, 0);
  if (total > 100) {
    total -= 5;
  }
  console.error("payment failed");
  return total;
}
```

`console.error` survives because it is not in the default method list. Set `methods` to `all` and
`keep` to `error,warn` to invert that: strip every console call except the ones you still want in
production.

### Choosing what happens to each statement

- **Delete the statement** (`action = remove`) — the default. When the statement was the only
  thing on its line, the whole line goes, so you do not get a trail of empty lines.
- **Comment it out** (`action = comment`) — each statement is prefixed with `//`, keeping the
  original indentation. Useful when you want the removal to be obvious in a code review.
- **Blank the line** (`action = blank`) — the statement is replaced by empty lines, so every later
  line keeps the number it had. Handy when you are matching output against a stack trace.

Set **Output** to *Dry-run report* to see what would change without changing anything: the report
lists the line number and text of every statement that would be removed, a per-method tally, and
the calls the tool deliberately left in place.

### Limits and edge cases

- Maximum input is 500,000 characters.
- Calls used as a **value** are never removed, because deleting them would change what the
  surrounding expression evaluates to. That covers `const a = console.log(x)`,
  `x && console.log(y)`, arrow bodies like `items.forEach(i => console.log(i))`, and chained calls
  such as `console.log(x).foo`. The dry-run report lists each one so you can decide by hand.
- A console call used as the un-braced body of `if` / `for` / `while` / `else` / `do` is replaced
  by an empty statement (`if (x) ;`) rather than deleted, so the control statement still parses.
- Only calls written on the `console` object are recognised, optionally through a
  `window.` / `globalThis.` / `self.` / `global.` receiver, and with optional chaining
  (`console?.log(x)`, `console.log?.(x)`). Aliases such as `const log = console.log; log(1)` need
  real scope analysis and are out of scope.
- Method names are validated: a typo like `warnn` is reported as an error rather than silently
  matching nothing.
- Source maps are not rewritten. Strip before you build, not after.
- `debugger;` removal is off by default; turn it on with the checkbox.

## FAQ

<details>
<summary>Does it delete a console.log that is written inside a string or a comment?</summary>

No. The scanner tracks string literals, template literals, regular expressions and both comment
styles, so only real calls in code positions are matched. A line such as
`const help = "call console.log(x) to debug";` comes back untouched, and so does
`// console.log(x)`.

</details>

<details>
<summary>How do I remove every console call except console.error?</summary>

Set **Console methods to strip** to `all` and **Never strip these** to `error`. The keep list wins
over the method list, so it behaves like an exclude list. Add `warn` to the keep list too if you
want warnings to survive the build.

</details>

<details>
<summary>Why is one of my console.log calls still there?</summary>

Because removing it would change behaviour. If the call is used as a value — assigned to a
variable, an operand of `&&`, the body of an arrow function, or the start of a chain — the tool
leaves it alone rather than silently altering the expression. Switch **Output** to *Dry-run
report* and it will be listed under the "Kept" heading with its line number.

</details>

<details>
<summary>Can I keep the line numbers stable?</summary>

Yes. Choose **Blank the line (keep line numbers)** for the action. Each removed statement is
replaced by the same number of blank lines it occupied, so line 200 is still line 200 afterwards.
Choose **Comment it out** instead if you would rather keep the statement readable.

</details>

<details>
<summary>Does it work on TypeScript, JSX and TSX?</summary>

Yes. The scanner works at the token level, and type annotations, generics and JSX do not change
how strings, comments or call parentheses are written, so `.ts` and `.tsx` sources are handled the
same way as `.js`. It is not a type checker — it only rewrites the console statements it matches.

</details>

<details>
<summary>Is my source code uploaded anywhere?</summary>

No. The tool is compiled to WebAssembly and runs entirely inside your browser tab. The code you
paste is never sent to a server, which also means it works offline once the page has loaded.

</details>
