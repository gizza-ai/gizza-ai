# file-tree-generator — competitor analysis (2026-06-23)

Tool: turn a list of file paths or an indented outline into an ASCII tree diagram
for READMEs and docs. Pure-Rust, browser-local, three surfaces (chat / CLI / page).

## Top competitors surveyed

1. **tree.nathanfriend.com** ("ASCII Tree Generator", `nathanfriend/tree-online`) —
   the most-linked free web tool. Indentation-based outline input → live ASCII tree.
   Options: trailing slash for directories, "fancy" (Unicode `├──`) vs plain ASCII
   connectors, fullwidth/half-width spacing. Output is copy-paste; runs client-side.
2. **`tree` (GNU/BSD CLI)** — the canonical reference for what a directory tree
   "should" look like (`├──`/`└──`/`│`, `-A` ASCII mode, `--dirsfirst`, `-L` depth).
   It walks a real filesystem, which a browser tool cannot.
3. **woochanleee/project-tree-generator** & VS Code "ASCII Tree Generator"
   extension — generate a tree from an actual folder you point them at (filesystem
   access). Output matches `tree`.
4. **ascii-tree (npm) / `tree-cli`** — take an indented/bulleted outline and emit a
   box-drawing tree; some accept a paths list.
5. **Generic "directory structure" web generators** (e.g. durgaprasadbudhwani,
   various devtool sites) — outline textarea → tree, usually Unicode-only, often
   ad-supported, server round-trip.

## Capability diff (us vs. them)

| Capability | nathanfriend | `tree` CLI | project-tree-gen | gizza file-tree-generator |
| --- | --- | --- | --- | --- |
| Outline (indentation) input | ✅ | ❌ | ❌ | ✅ |
| Paths-list input (merge shared prefixes) | ❌ | ❌ (walks FS) | ❌ | ✅ |
| Unicode box-drawing output | ✅ | ✅ | ✅ | ✅ |
| Plain-ASCII connectors | ✅ | ✅ (`-A` is inverse) | partial | ✅ (`ascii`) |
| Trailing slash on directories | ✅ | ✅ (`-F`) | ✅ | ✅ (`trailing_slash`, default on) |
| Directories-first / sorted | ❌ | ✅ (`--dirsfirst`) | ✅ | ✅ (`sort`) |
| Custom root label | partial | ✅ (the path) | ✅ | ✅ (`root`) |
| Tabs + spaces mixed in outline | ✅ | n/a | n/a | ✅ (tab = 4 cols) |
| Windows `\` paths accepted | ❌ | n/a | ✅ | ✅ |
| Fully client-side / private | ✅ | n/a (local) | n/a | ✅ |
| Available as chat tool + CLI + page | ❌ | CLI only | ❌ | ✅ (3 surfaces) |

## Gaps closed in this build (in-model)

- **Paths-list mode** — the differentiator vs nathanfriend: paste raw output of
  `git ls-files` / `find` and the tool merges shared folders. nathanfriend only
  does the outline mode; gizza does **both** (`mode` param).
- **Directories-first sort** (`sort`) — matches `tree --dirsfirst`; nathanfriend
  lacks it.
- **Plain-ASCII vs Unicode connectors** (`ascii`) — parity with nathanfriend's
  "fancy/basic" toggle and `tree -A`.
- **Trailing-slash toggle** + **custom root label** — parity with `tree -F` and
  the CLI's root path.
- **Windows backslash paths** + **mixed tabs/spaces** — robustness several web
  tools lack.

## Out-of-model features (deliberately NOT built)

- **Walking a real folder / filesystem access** (`tree`, the VS Code extension,
  project-tree-generator): gizza is browser-local and has no filesystem input in
  the page/CLI model — the user pastes the paths/outline instead. This is the core
  architectural boundary, not a missing feature.
- **Drag-and-drop a folder to enumerate it**: would need a directory-upload page
  input that the single-field page driver doesn't provide. Out of model.
- **File-size / line-count annotations** (`tree -h`): require reading file
  contents, which we don't have. Out of model.
- No competitor copy, branding, or trademarks were used; only capability parity
  was assessed.

## Verification (this build)

- Unit tests: 15 core + 1 descriptor drift-guard — all pass.
- `wafer build`: chat block validates (319.7 KiB), instantiates clean.
- CLI: `gizza tool file-tree-generator` verified for paths mode, ascii+sort,
  and the empty-input error (exit 1).
- Page: Playwright `tool-page-file-tree-generator.spec.ts` — 3/3 pass (paths
  default, outline mode, ASCII connectors).
- wafer fixtures: paths-basic, outline-basic, ascii-mode, empty-error.
