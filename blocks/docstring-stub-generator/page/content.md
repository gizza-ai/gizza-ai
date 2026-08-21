## About this tool

Paste a function signature and get back a documentation stub with a slot for every parameter, the return value and the errors it raises. Nothing is uploaded — the parser is compiled to WebAssembly and runs inside your browser tab.

Ten languages are covered, each in its own convention: Python docstrings (Google, NumPy, Sphinx/reStructuredText, Epytext and PEP 257), JSDoc for JavaScript and TypeScript, PHPDoc, Javadoc, C# XML doc comments, godoc, rustdoc and Ruby YARD. Leave the language on **Auto-detect** and the shape of the code decides.

Parameter types come from the annotations you wrote; where there is no annotation, the default value is used as a hint (`timeout = 30` → `int`, `name = "x"` → `str`), and anything still unknown becomes a `_type_` placeholder you can replace. Set **Types** to *Declared annotations only* to suppress the guessing, or to *No types* for the untyped variant of each convention.

### Worked example

Input:

```python
def fetch(url: str, timeout: int = 30) -> dict:
```

With `language = python`, `style = google` and the default options, the output is:

```python
def fetch(url: str, timeout: int = 30) -> dict:
    """_description_

    Args:
        url (str): _description_
        timeout (int, optional): _description_. Defaults to 30.

    Returns:
        dict: _description_
    """
```

Switch **Language** to `typescript`, turn **Align tag columns** on, and `export async function load(id: string, opts?: LoadOptions): Promise<User> {` produces a JSDoc block above the signature instead, with the descriptions lined up:

```js
/**
 * _description_
 *
 * @param {string}      id     - _description_
 * @param {LoadOptions} [opts] - _description_
 * @returns {Promise<User>} _description_
 */
```

Set **Output** to *Stub only* for just the comment block, or to *Parsed signature as JSON* for a machine-readable `{name, async, params, returns, raises}` record — handy for feeding a codemod.

## Limits and edge cases

This reads **signatures**, not function bodies, so it cannot know which errors a function throws: list them yourself in the **Errors raised** box and they become a `Raises:` / `@throws` / `# Errors` section. Java `throws` clauses are the one exception — they are read straight off the signature.

Descriptions are always placeholders. This tool does not write prose for you; it builds the scaffolding so the summary is the only thing left to type.

It is a structural parser, not a full language grammar. Destructured JavaScript parameters (`function f({a, b})`) are documented as a single parameter under their literal text, unnamed Go parameters are treated as names, and a trailing comment on the same line as a signature is read as part of it. Input is capped at 200,000 bytes and 200 signatures per run, with 100 parameters per signature and 20 declared error names.

## FAQ

<details>
<summary>Which docstring conventions are supported for Python?</summary>

Google, NumPy, Sphinx/reStructuredText, Epytext and PEP 257, chosen with the **Python docstring convention** control. *Auto* means Google. The quote style control switches between `"""` and `'''`. These settings are ignored for the other nine languages, which each have exactly one native convention.

</details>

<details>
<summary>Can I paste a whole function, not just its signature?</summary>

Yes. Every line that is not a signature is passed through untouched, so the body comes back unchanged with the stub inserted in the right place — inside the `def` for Python, above the signature for the comment-block languages. You can also paste several signatures at once and each one gets its own stub.

</details>

<details>
<summary>Where do the types come from if my code has no annotations?</summary>

From the default values. `retries = 3` is documented as an `int`, `label = "x"` as a `str`, `opts = {}` as a `dict`, and so on, using each language's own type names. Parameters with neither an annotation nor a default fall back to a placeholder such as `_type_`, `*` or `mixed`. Choose *Declared annotations only* to leave those slots empty instead, or *No types* to drop the type columns entirely.

</details>

<details>
<summary>Will it write the descriptions for me?</summary>

No — and that is deliberate. This is a deterministic parser with no language model behind it, so it never invents claims about what your code does. It fills every description slot with the placeholder text you choose (`_description_` by default, or something like `TODO` that you can grep for later).

</details>

<details>
<summary>Why did auto-detection pick the wrong language?</summary>

A few signatures are genuinely ambiguous — `def f(a)` is valid Ruby and near-valid Python, and a bare `Type name(args)` line could be Java or C#. Detection uses trailing colons, sigils, keywords and type names to decide. When it guesses wrong, set the **Language** control explicitly; that overrides detection completely.

</details>
