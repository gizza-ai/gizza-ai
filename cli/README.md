# gizza-cli

Headless CLI to run [gizza](https://gizza.ai) skill tools locally — calculators, clocks, image/video processing, web fetch, and more — without a browser or GPU.

## Requirements

- Rust 1.82+ (stable)
- System `ffmpeg` on `PATH` for image and video tools (`image-resize`, `image-convert`, `image-crop`, `image-fetch`, `video-trim`, `video-transcode`, `video-frame-extract`, `ffmpeg`)
- The skill WASMs must be pre-built (run `solobase build` from the `gizza-ai` repo root before using or testing)

> **Note on `imagine`:** The `imagine` (text-to-image) tool requires a browser GPU and is not supported in the CLI. Invoking it returns exit code 3 (`unsupported_in_cli`). Use [gizza.ai](https://gizza.ai) instead.

## Build

```sh
# From gizza-ai/ repo root:
solobase build                      # compile all skill WASMs
cargo build --manifest-path cli/Cargo.toml
```

## Invoking tools

```sh
# Positional: bare value fills the first required scalar field
gizza tool calculator "2+2"
# → 4

# Key=value pairs (any order, mixed with positionals)
gizza tool calculator expr="6*7"
# → 42

# Full JSON body
gizza tool calculator --json '{"expr":"sqrt(144)"}'
# → 12

# Print the complete JSON response envelope
gizza tool calculator "2+2" --json-out

# Write binary output (image, video) to a file
gizza tool image-resize url=https://example.com/cat.png width=64 --out thumbnail.png
```

## Discovery

```sh
gizza list                          # table: short-name  description
gizza list --json-out               # JSON array of {name, description, parameters}

gizza describe calculator           # schema for one tool
gizza describe calculator --json-out
```

## Generate / check SKILL.md

`SKILL.md` at the repo root is a machine-readable contract (YAML frontmatter + Markdown) that teaches an LLM agent how to drive the CLI. It is generated from live tool metadata.

```sh
# (Re)write SKILL.md — run from the gizza-ai repo root:
cargo run --manifest-path cli/Cargo.toml --bin gizza -- gen-skill

# Check that the committed SKILL.md is up to date (CI / maintainers):
cargo run --manifest-path cli/Cargo.toml --bin gizza -- gen-skill --check
```

After changing tool metadata (descriptions, parameter schemas, adding/removing tools), re-run `gen-skill` and commit the updated `SKILL.md`.

## Exit codes

| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | Tool error (invalid input, compute failure) |
| 2    | Usage error (unknown tool, missing required arg) |
| 3    | Unsupported in CLI (`imagine` requires browser GPU) |

## Embedding the library

Other Rust programs can embed the tools directly without the binary:

```toml
# Cargo.toml
gizza-cli = { path = "../gizza-ai/cli" }
```

```rust
use gizza_cli::{run_tool, list_tools, describe_tool};

// Run a tool by short name with JSON args, returns response body bytes.
let body = run_tool("calculator", serde_json::json!({"expr": "6*7"})).await?;

// List all tools.
let tools = list_tools().await?;

// Describe one tool.
let meta = describe_tool("calculator").await?;
```

See `SKILL.md` for the full agent contract (all tool schemas and CLI examples).

## Tests

```sh
# From gizza-ai/cli/:
cargo test
```

All integration tests require the skill WASMs to be pre-built (`solobase build`).
