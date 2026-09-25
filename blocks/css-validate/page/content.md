## About this tool

Paste CSS and get a local validation report with 1-based line and column numbers. The validator checks rule structure, braces, comments, strings, selector basics, declaration syntax, common property names, vendor-prefixed properties, malformed hex colors, `calc()` syntax, and `var(--name)` references.

Use `format=json` when you need machine-readable output for editor integrations or CI logs. The report mode is easier to read while editing because it groups a validity summary, optional stats, and individual diagnostics.

### Worked example

Input:

```css
.hero, {
  colr: #12zz width: 10px;
}
```

Typical report output includes an empty selector error, an unknown `colr` property warning, a malformed hex color error, and a possible missing semicolon warning.

### Limits and edge cases

This is a lightweight syntax and diagnostics checker, not a full browser CSS engine. It uses a curated modern property list and catches common value mistakes, but it does not implement every per-property grammar, shorthand expansion rule, profile/level switch, or browser compatibility matrix. Unknown properties default to warnings so newly standardized properties do not automatically make a stylesheet invalid; choose "Error" for stricter linting.

## FAQ

<details>
<summary>Does this validate CSS values as deeply as a browser?</summary>

No. It catches malformed declaration structure, bad hex colors, suspicious `calc()` and `var()` usage, and common typos, but it does not include every value grammar for every CSS property. For example, complex `background` shorthand ordering is reported only when the syntax shape is clearly broken.

</details>

<details>
<summary>How are unknown properties handled?</summary>

The default is a warning based on a curated list of modern CSS properties. You can switch unknown properties to "Error" for strict linting or "Ignore" when checking experimental CSS. Custom properties such as `--brand-color` are always accepted.

</details>

<details>
<summary>Why are vendor-prefixed properties separate?</summary>

Vendor prefixes such as `-webkit-` can be intentional compatibility code. The tool therefore has a dedicated vendor-prefix setting: ignore them by default, warn when you want cleanup guidance, or treat them as errors for strict codebases.

</details>

<details>
<summary>Is my CSS uploaded anywhere?</summary>

No. The page runs the validator in WebAssembly in your browser, and the CLI/chat block runs the same Rust core locally. There is no network fetch or remote validation step.

</details>
