## About this tool

**HTML to JSX Converter** turns plain HTML into React-ready JSX. It is the step you hit every time you paste markup from a design export, a CMS, a Bootstrap/Tailwind snippet, an email template, or an old server-rendered template into a React component and the compiler starts complaining about `class`, unclosed `<img>` tags, and string `style` attributes.

The converter rewrites everything JSX is strict about:

- **React attribute names** — `class` → `className`, `for` → `htmlFor`, `tabindex` → `tabIndex`, `readonly` → `readOnly`, `maxlength` → `maxLength`, `cellpadding` → `cellPadding`, `http-equiv` → `httpEquiv`, plus the SVG set (`stroke-width` → `strokeWidth`, `viewbox` → `viewBox`, `xlink:href` → `xlinkHref`). `data-*` and `aria-*` stay hyphenated, because React wants them that way.
- **Inline styles** — a `style="color: red; font-size: 12px"` string becomes a `style={{ color: "red", fontSize: "12px" }}` object with camelCased properties and string values. Vendor prefixes follow React's rules (`-webkit-box-shadow` → `WebkitBoxShadow`, `-ms-flex` → `msFlex`) and CSS custom properties keep their name as a quoted key (`"--brand"`).
- **Boolean attributes** — `disabled`, `readonly`, `required` and friends become `disabled={true}`, or stay bare if you prefer the shorthand.
- **Void tags** — `<br>`, `<img>`, `<input>`, `<hr>` and the rest are self-closed as `<br />`.
- **Comments** — `<!-- note -->` becomes `{/* note */}`, or is removed.
- **Safe text** — literal `{` and `}` in text are escaped as `{'{'}` / `{'}'}` so they aren't parsed as expressions.
- **Inline handlers** — `onclick="save()"` becomes `onClick={() => { save() }}` so React gets a function, not a string.
- **Multiple roots** — several top-level nodes are wrapped in a `<>…</>` fragment, which JSX requires.

### Worked example

Input HTML:

```html
<div class="card"><label for="n">Name</label><input id="n" tabindex="2" readonly></div>
```

JSX output:

```jsx
<div className="card">
  <label htmlFor="n">Name</label>
  <input id="n" tabIndex={2} readOnly={true} />
</div>
```

With **Wrap in a component** set to `Card` and 4-space indentation, `<p style="color: red; font-size: 12px">Hi</p>` becomes:

```jsx
export default function Card() {
    return (
        <p style={{ color: "red", fontSize: "12px" }}>Hi</p>
    );
}
```

### Options

- **Indentation** — 2 spaces (default), 4 spaces, or tabs.
- **Wrap in a component** — leave empty for a bare JSX snippet, or give a name to get `export default function Name() { return (…); }`.
- **HTML comments** — keep them as `{/* … */}` expression containers, or drop them.
- **Boolean attributes** — `disabled={true}` (explicit, the default) or bare `disabled`.
- **value / checked on form fields** — rewrite to `defaultValue` / `defaultChecked` on `input`, `textarea` and `select` (default) so React doesn't warn about an uncontrolled field that never updates, or keep the original names when you're wiring up controlled inputs yourself.

### Limits and edge cases

- The parser is a forgiving HTML tokenizer, not a full HTML5 tree builder. It handles unclosed tags, implicit `</li>` / `</p>` / `</td>` closes, unquoted attribute values and stray text, but it does not reconstruct a document the way a browser would (no table-scoping repair, no foster parenting).
- Whitespace between elements is collapsed the way a browser renders it, so the output is indented and readable. `<pre>` and `<textarea>` content is preserved verbatim.
- `<script>` and `<style>` contents are emitted verbatim inside a JSX template-literal child, so the CSS or JS is preserved exactly. That compiles, but for real projects a stylesheet import or `dangerouslySetInnerHTML` is usually the better home.
- A `<!DOCTYPE html>` declaration has no JSX equivalent and is dropped.
- Inline `on*` handlers are wrapped in an arrow function verbatim — the JavaScript inside is not analysed, so the referenced functions still have to exist in your component's scope.
- HTML entities such as `&nbsp;` and `&amp;` are left as-is: JSX decodes them in text and in attribute strings.

## FAQ

<details>
<summary>Why does JSX need `className` instead of `class`?</summary>

JSX compiles to JavaScript, and `class` is a reserved word there, so React named the DOM property `className` (the same name the browser's DOM API uses). `for` has the same problem — it becomes `htmlFor`. This tool renames both, along with the rest of React's camelCase attribute set.

</details>

<details>
<summary>Why did my `style="…"` string turn into double braces?</summary>

React expects a JavaScript object for `style`, not a CSS string. The outer braces are the JSX expression container and the inner ones are the object literal, which is why it reads as `style={{ … }}`. Property names are camelCased (`font-size` → `fontSize`) and values stay strings, so units like `px` and `%` survive untouched.

</details>

<details>
<summary>What happens to `value` and `checked` on a form field?</summary>

By default they become `defaultValue` and `defaultChecked`. React treats `value` / `checked` as a *controlled* value: without an `onChange` handler the field would be frozen and React would log a warning. `defaultValue` / `defaultChecked` gives you the same initial state with an editable field. Switch the option to **Keep as value / checked** if you're adding state and a change handler yourself.

</details>

<details>
<summary>Can I convert a whole HTML page?</summary>

Yes — paste it and you'll get the `<html>`/`<head>`/`<body>` tree as JSX, with the doctype dropped. In practice you usually want one component's worth of markup: React apps rarely render `<html>` directly outside a framework's document file, and a full page pulls in `<meta>`/`<script>` tags that belong in your framework's head configuration.

</details>

<details>
<summary>Is my HTML uploaded anywhere?</summary>

No. The page runs a WebAssembly converter in your browser. The markup is parsed and converted locally, and you can copy or download the JSX without the content leaving your device.

</details>

<details>
<summary>Does it handle SVG icons?</summary>

Yes. SVG attributes are camelCased the way React expects (`stroke-width` → `strokeWidth`, `stroke-linecap` → `strokeLinecap`, `viewbox` → `viewBox`, `xlink:href` → `xlinkHref`), and camelCase SVG element names such as `<linearGradient>` and `<clipPath>` are preserved even if the source wrote them in lowercase.

</details>
