## What this tool does

This tool turns small TypeScript snippets into best-effort JavaScript by removing syntax that only exists for the type system. It strips annotations, interfaces, type aliases, `as`/`satisfies` assertions, optional markers, access modifiers, `implements` clauses, type-only imports/exports, and simple `enum` declarations.

The conversion runs locally in WebAssembly. It is useful for examples, docs, playground snippets, and quick migrations where you want readable JavaScript without starting a full TypeScript compiler.

## Example

Input:

```ts
type User = { name: string }
export function greet(name: string): string {
  const label: string = name as string;
  return `Hello ${label}`;
}
```

Output:

```js
export function greet(name) {
  const label = name;
  return `Hello ${label}`;
}
```

## Options

| Option | Choices | What it does |
| --- | --- | --- |
| **Enum handling** | `compile` (default), `strip` | Converts simple enums into JavaScript objects, or removes enum declarations entirely. |
| **Remove comments** | off (default), on | Removes `//` and `/* ... */` comments after normalizing line endings. |

## Limits and edge cases

This is a deterministic syntax stripper, not the official TypeScript compiler. It does not type-check, resolve modules, inline `const enum`, transform namespaces, emit decorator metadata, rewrite JSX, downlevel modern JavaScript, or guarantee correct output for every legal TypeScript program. For production builds, use `tsc`, Babel, SWC, or esbuild.

The input limit is 1 MB. The output preserves most original formatting, so already-minified input stays compact.

## FAQ

<details>
<summary>Is this the same as running tsc?</summary>

No. `tsc` parses the whole TypeScript language, type-checks when requested, resolves project settings, and can downlevel JavaScript. This tool is a fast local syntax stripper for snippets and simple files.

</details>

<details>
<summary>Does it support interfaces and type aliases?</summary>

Yes. Interface and type alias declarations are removed because they have no JavaScript runtime representation. References to those types in annotations are removed with the annotation.

</details>

<details>
<summary>What happens to enums?</summary>

By default, simple numeric enums are converted to JavaScript objects. If you choose `strip`, enum declarations are removed instead. Complex enum initializers are copied as object values but are not evaluated.

</details>

<details>
<summary>Can I use the output in production?</summary>

Use it as a preview or migration helper, then run your normal compiler and tests. The tool deliberately documents unsupported TypeScript features rather than pretending to be a complete compiler.

</details>
