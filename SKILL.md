---
name: gizza-tools
description: "Run gizza's local compute tools (calculator, text, image/video, fetch, …) from the terminal via the `gizza` CLI. The tool set is live — discover it with `gizza list`; never hardcode it."
---

# gizza tools (CLI)

The `gizza` binary runs gizza's tools headlessly — math, text, image/video (ffmpeg),
URL fetch, and more. Each tool runs the exact same wasm the chat uses.

## Discover the tools (do this first — the set is live, don't assume a fixed list)

- `gizza list` — every available tool, with a one-line description.
- `gizza describe <name>` — a tool's input JSON Schema (add `--json-out` for raw JSON).

## Run a tool

- `gizza tool <name> "<arg>"` — positional args fill the schema's required scalar fields, in order.
- `gizza tool <name> key=value …` — named args (coerced to the schema's types).
- `gizza tool <name> --json '{…}'` — pass the full JSON body (mutually exclusive with positional/key=value).
- `--json-out` — print the full `{_for_llm, _for_ui}` JSON envelope instead of human text.
- `--out <file>` — write a binary result (image/video) to a file instead of stdout.

Examples:

```sh
gizza tool calculator "2*2"                          # → 4
gizza tool web-fetch url=https://example.com
gizza tool image-resize url=https://…/cat.png width=640 --out cat.png
```

## Exit codes

`0` ok · `1` tool error · `2` usage / bad args · `3` unsupported in the CLI (e.g. GPU
tools like `imagine`, which need a browser GPU).

## MCP server

`gizza mcp` serves the same tool set over the Model Context Protocol (stdio,
newline-delimited JSON-RPC) — register it with an MCP client instead of shelling out:
`claude mcp add gizza -- /path/to/gizza mcp`. See `cli/README.md` for details.

## Notes

- Image/video tools shell out to the system `ffmpeg` binary — install it to use them.
- New tools are added over time, so always `gizza list` for the current set rather than
  relying on this document.
