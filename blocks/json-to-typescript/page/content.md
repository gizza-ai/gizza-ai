## JSON to TypeScript interfaces, in your browser

Paste a JSON object or array and get ready-to-use **TypeScript** interfaces.
Everything runs locally in your browser — your JSON is never uploaded.

### What it infers

- **Nested objects** become their own named interfaces (keyed off the field name).
- **Arrays of objects** are merged: a key missing in some elements becomes
  **optional** (`?`), and differing value types become a **union** (e.g.
  `number | string`).
- **Primitives** map to `string` / `number` / `boolean` / `null`; empty arrays
  become `any[]`.
- Non-identifier keys are quoted (`"first-name": string`).

### Options

- **Root type name** — names the top-level interface (default `Root`). A
  top-level array yields `export type Root = RootItem[];`.
- **Add 'export'** — prefix declarations with `export` (on by default).

## FAQ

<details>
<summary>What happens when objects in an array don't all have the same keys?</summary>

They're merged structurally into one interface. A key that's missing from some
elements becomes **optional** (`name?:`), and a key whose values have different
types across elements becomes a **union** (`id: number | string`). The generated
interface therefore describes *every* element you pasted, not just the first one.

</details>

<details>
<summary>How are the interface names picked?</summary>

The top-level name comes from the **Root type name** option (PascalCased,
default `Root`). Nested objects are named after their field key — a `user` field
produces `interface User` — and elements of an array take the singular of the
array's key, so an `items` array yields `interface Item`. If two shapes would
claim the same name, a suffix keeps them distinct.

</details>

<details>
<summary>How are null values and empty arrays typed?</summary>

A JSON `null` maps to the TS type `null`; when the same key is `null` in one
sample and a string in another, you get `string | null`. An empty array carries
no element information, so it becomes `any[]` — paste a sample with at least one
element if you want a precise type.

</details>

<details>
<summary>Why did I get a <code>type</code> alias instead of an <code>interface</code>?</summary>

Objects become `interface` declarations, but a top-level **array** (or primitive)
can't be an interface, so the tool emits a type alias — e.g.
`export type Root = RootItem[];` — alongside the interfaces for the element
shapes. Untick **Add 'export'** if you want bare declarations for pasting inside
an existing module.

</details>
