# html-to-jsx competitor analysis (2026-08-08)

Tool: `html-to-jsx` — convert HTML snippets into React JSX.

## Sources scanned

Search query: `HTML to JSX converter online className style object self closing tags React`.

Top relevant results reviewed from snippets/search descriptions:

1. Folge HTML to JSX Converter — advertises standard tags, attributes, inline styles, class names, event handlers, self-closing tags, and formatting; notes complex patterns may need manual adjustment.
2. js2ts HTML to JSX — table stakes include `className`, event handlers, self-closing tags, and inline styles.
3. Hexa Tools HTML to JSX — focuses on `className`, `htmlFor`, and self-closing tags.
4. Online Tools Forge HTML to JSX/TSX Converter — transforms HTML into React components and adjusts `class`, `for`, inline style objects, and self-closing tags.
5. AI Dev Hub HTML to JSX — mentions attribute renaming, style object conversion, self-closing tags, and comment syntax.

## Table-stakes capabilities

| Capability / UX pattern | In current gizza model? | Decision |
| --- | --- | --- |
| Paste HTML snippet in a text area | Yes | Build as required multiline `html` parameter. |
| `class` → `className` | Yes | Core attribute mapping. |
| `for` → `htmlFor` | Yes | Core attribute mapping. |
| React casing for common attributes (`tabindex`, `readonly`, `maxlength`, SVG attrs) | Yes | Include broad mapping plus hyphen-to-camel fallback for SVG-style attrs. |
| Inline `style="..."` string → JSX object | Yes | Parse declarations, camelCase CSS property names, quote string values. |
| Self-close void tags (`img`, `br`, `input`, etc.) | Yes | Void element table and JSX printer. |
| HTML comments → JSX comments | Yes | Default `comments=jsx`; expose `comments=strip`. |
| Event handlers | Partly | Convert inline `onclick="save()"` to a function wrapper (`onClick={() => { save() }}`); do not analyze or validate handler JS. |
| Optional component wrapper | Yes | `component` parameter returns `export default function Name() { return (...); }`. |
| Indentation controls | Yes | Enum for 2 spaces, 4 spaces, or tabs. |
| Boolean attr style | Yes | `boolean_attrs=explicit/shorthand`. |
| Form value/checked rewriting | Yes | Default to `defaultValue`/`defaultChecked` to avoid uncontrolled-field warnings; expose `value_attrs=keep`. |
| Full browser-grade HTML5 tree repair | No | Out-of-model for a dependency-free wasm-safe converter; document as a limitation. |
| TSX type generation / props interfaces | No | Out of scope; this tool transforms markup, not component API design. |

## Defaults chosen

- `indent=2` — common in React examples and existing site snippets.
- `comments=jsx` — preserving comments is less lossy than dropping them.
- `boolean_attrs=explicit` — easy to see exactly what changed and exercises a non-default shorthand option separately.
- `value_attrs=default` — avoids React warnings for copied form markup.
- Empty `component` — bare JSX snippet is the smallest expected output; component wrapping is optional.

## Examples and controls

The page includes chips for common form markup, inline styles wrapped in a component, SVG icon conversion, and stripped comments with shorthand booleans. Enum controls use labels so users can choose formatting and React-specific rewrite policies without memorizing internal values.
