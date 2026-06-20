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
