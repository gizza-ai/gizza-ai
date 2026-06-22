## About this tool

**JSON Merge** deep-merges two or more JSON documents into one. Paste your JSON
values one after another (separated by whitespace or a newline) — they're merged
**left to right**.

The merge rules:

- **Objects** are merged **recursively** — nested objects combine key by key.
- **Conflicts** (a key set to different scalar/types) are resolved
  **last-wins**: the later document's value replaces the earlier one.
- **Arrays** are **replaced** by the later value by default; tick **Concatenate
  arrays** to append them instead.

Set **Indent** for the output (or 0 to minify). Everything runs **locally in your
browser** via WebAssembly — your data is never uploaded.

### Handy for

- Layering a base config with environment-specific overrides.
- Combining partial JSON responses or fixtures.
- Applying a "patch" object on top of a default.
