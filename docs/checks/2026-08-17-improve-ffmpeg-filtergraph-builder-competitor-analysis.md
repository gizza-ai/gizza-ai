# ffmpeg-filtergraph-builder — competitor analysis (2026-08-17)

Scan run **before** implementing (per `/create-next-tool` step 4). All notes are **paraphrased
observations of function**; no competitor copy, branding, or trademarks were reused.

## Model-fit decision (checked first)

The backlog row is tagged `video`, but the row's own note says *"plain Rust→WASM — composes and
syntax-validates the filtergraph string, no execution required"*. Confirmed: this tool takes
**text in → text out** and **never runs ffmpeg** and never touches media, so it is built as a
**pure** block (`Input::None`), not an ffmpeg block.

Safety posture, deliberately chosen up front:

- The tool **emits a string only** — no process is spawned, no user-supplied filter is executed
  anywhere in this repo or on the page.
- The `command` output form is a *copy-me* line. Filenames are validated against a strict
  allowlist (`A-Za-z0-9._/-`), so shell metacharacters (`` ; | & $ ` " ' < > \ * ? ( ) `` space,
  newline) are rejected with a clear error instead of being interpolated into a command line.
- `drawtext` text is emitted with `expansion=none` and rejects `'`/`\`/control characters, so
  neither filtergraph-level escaping nor drawtext's `%{…}` expansion can be smuggled through.
- The assembled graph is validated (balanced quotes/brackets, no stray `;`, no control chars,
  label charset) before it is returned.

**Not a duplicate.** `blocks/ffmpeg` runs ffprobe-style inspection on a media URL; every
`blocks/video-*` / `blocks/audio-*` block *applies* one fixed transform to an uploaded file.
Nothing in `blocks/` composes a filtergraph *string* from a step list. Closest precedent in
shape is `blocks/http-request-builder` (builds the bytes, sends nothing) — same "builder, not
executor" family.

## Competitors reviewed

| # | Tool | Shape | Reachable |
|---|------|-------|-----------|
| 1 | FFmpeg Explorer (lav.io / ffmpeg.lav.io) | Web, node-graph editor + in-browser render | yes |
| 2 | FFmpeg Commander (alfg) | Web form → full ffmpeg command | yes (title only; behavior from its author's write-up + search summary) |
| 3 | `filter-complex-graph` (JS library) | Programmatic graph builder | yes |
| 4 | `python-ffmpegio.filtergraph` | Programmatic Filter/Chain/Graph classes | yes |
| 5 | `typed-ffmpeg` (Python) | Typed programmatic filter API | listed only (metadata-level) |

### 1. FFmpeg Explorer
Visual node editor: the user chains an input node to filter nodes, each filter exposing its
parameters with min/max/defaults pulled from ffmpeg's filter metadata. Generates a copy-paste
command, can render preview output in-browser, ships demo videos and example configurations, and
can export the graph as JSON. Auto-connects newly added nodes by default with a layout lock the
user can release for manual routing. Its own notes state the command generation is incomplete for
complex graphs.

### 2. FFmpeg Commander
Form-driven generator (container / codec / video / audio settings plus a handful of basic
filters) that prints a complete `ffmpeg …` command with a copy button and local-storage save. Its
value is "pick options, get a runnable command" rather than filtergraph composition per se.

### 3. `filter-complex-graph`
Takes an array of chain objects — `inputs` (stream labels like `0:v`), `filters`
(`{name, options}`), and optional `outputs` (labelled pads) — and stringifies them into
`[0:v]fade=type=in:st=0:duration=1,scale=512:-2` form, with `;` between chains. Options may be an
object or a raw string. No documented validation.

### 4. `python-ffmpegio.filtergraph`
Filter / Chain / Graph classes with composition operators (stack, join, label/link),
**auto-generated link labels**, `str()` stringification, and **validation of filter names and
options against the installed ffmpeg**, with helpful error messages on bad options or pad
connections.

### 5. `typed-ffmpeg`
Typed Python API over ffmpeg filters with per-filter documentation; same "compose in code"
category as 3 and 4.

## Table stakes → decision

| Capability seen in competitors | Decision | Where |
|---|---|---|
| Emit a `filter_complex` string with `[in]`/`[out]` link labels | **built** | `output = filter_complex` (default), `input_label`, `output_label` |
| Emit the plain comma chain for `-vf`/`-af` | **built** | `output = filter_chain` |
| Emit a complete runnable `ffmpeg …` command | **built** | `output = command` (+ `input_file`, `output_file`; `-map` keeps the other stream when present) |
| Common video filters with sane defaults (scale, crop, pad, fade, rotate, flip, fps, speed, trim, blur, sharpen, eq, grayscale, reverse, drawtext) | **built** | 17 video step keywords |
| Audio filters (volume, fade, trim, speed, normalize, mono, highpass, lowpass, reverse) | **built** | `stream = audio` |
| Per-filter parameter defaults so a bare step still works | **built** | e.g. `fade in` → `fade=t=in:st=0:d=1`, `blur` → `gblur=sigma=5` |
| Presets / example configurations (Explorer's examples, Commander's presets) | **built** | four `[[example]]` chips on the page |
| Copy-to-clipboard of the generated command | **built** | generator ships Copy + Reset on every text page |
| Escape hatch for a filter the builder doesn't model (Explorer exposes "most" filters) | **built** | `raw <filter>` step, syntax-validated, still never executed |
| Validation with helpful messages (python-ffmpegio) | **built** | per-line errors naming the line, what was expected, and what was received; graph re-validated after assembly |
| Explain what each step compiled to | **built** (beyond competitors) | `explain` checkbox → `#` breakdown lines |
| Node-graph GUI with drag-to-connect | **out of model** | a canvas graph editor is a bespoke app UI, not a declarative generator control; the ordered step list is the in-model equivalent |
| In-browser video render / preview of the result | **out of model** | this tool takes no media input; the repo already has ~90 `video-*`/`audio-*` blocks that *apply* transforms |
| Multi-input graphs (overlay, concat, amix) | **out of model** | needs ≥2 media inputs — the same limit that skiplisted `video-concat`, `add-audio-to-video`, `mix-audio`. `raw` still lets a user hand-write one chain |
| Filter-name validation against a *locally installed* ffmpeg (python-ffmpegio) | **considered, rejected** | there is no ffmpeg in a wasm page; we validate the 26 modelled steps exactly and syntax-check `raw`, which is honest and offline |
| Export the graph as JSON (Explorer) | **considered, rejected** | the step list *is* the portable source, and page deep-links (`?steps=…`) already share a graph as a URL |

## Result

Built as a pure block with 8 parameters, 26 step keywords across video/audio, three output forms,
a validated `raw` escape hatch, and an `explain` breakdown. Every table stake above is either in
the descriptor or listed as out-of-model — none dropped silently.
