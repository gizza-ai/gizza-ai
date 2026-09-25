# harmony-format-renderer — competitor analysis (2026-08-29)

Scan run BEFORE implementing, per `/create-next-tool` step 4. Everything below is
**paraphrased**; no competitor copy, branding, or trademarks were copied. The one
exception is the Harmony format's own **literal control tokens and fixed header
strings** (`<|start|>`, `Knowledge cutoff:`, `# Valid channels: …`) — those are a
wire protocol, not copy: rendering them differently would produce output the model
cannot parse, so they are reproduced exactly by necessity.

Search: "OpenAI Harmony response format renderer tool gpt-oss conversation prompt builder".

## Competitor profiles

### 1. `openai/harmony` — the official renderer library (Rust core + Python bindings)

- **Features:** builds a typed `Conversation` of `Message`s and renders it to
  Harmony token IDs; also parses model completions back into messages. Ships the
  `o200k_harmony` tokenizer encoding.
- **Params/options:** `SystemContent` carries a model-identity line, a reasoning
  effort (`low`/`medium`/`high`, default medium), a knowledge cutoff (default
  `2024-06`), an optional conversation start date, a required-channel list
  (default `analysis`, `commentary`, `final`), and optional built-in tool
  namespaces. `DeveloperContent` carries free-text instructions plus a map of
  function-tool namespaces. Rendering entry points distinguish "render the whole
  conversation" from "render for completion" (append the generation prompt for a
  given next-turn role).
- **Input/output:** typed objects in → token IDs (or text) out.
- **UX:** library only — no UI. You write code, install a package, and get tokens.
- **Limits stated:** the tokenizer vocabulary is a large data file; the library is
  the recommended path over hand-rolling the format.
- **Free vs paid:** free/open source.

### 2. OpenAI Cookbook — "OpenAI Harmony Response Format" guide

- **Features:** the normative prose spec. Documents every control token with its
  token ID, the message-header grammar, role precedence
  (system → developer → user → assistant → tool), the three channels
  (`analysis` for chain-of-thought, `commentary` for tool calls and preambles,
  `final` for the user-facing answer), the function-tool namespace syntax, and the
  chain-of-thought drop rule.
- **Worked examples:** a bare system message; a system+developer+user prompt; a
  tool-calling round trip; the "replace a trailing `<|return|>` with `<|end|>`
  when storing history" rule.
- **UX:** documentation with copy-able code fences. No interactive renderer — the
  reader assembles the string by hand or installs the library.
- **SEO angles:** "what is the harmony format", "gpt-oss prompt format",
  "harmony channels analysis commentary final", "harmony special tokens".

### 3. Hugging Face Transformers `apply_chat_template` for gpt-oss

- **Features:** renders a plain OpenAI-style message list into a Harmony prompt via
  the model's packaged chat template.
- **Params/options:** `add_generation_prompt` (append the next-turn prompt),
  tensor/dict return shaping. Reasoning effort and tools are exposed unevenly
  depending on template version.
- **Key documented behavior:** the Harmony **developer** role is what a caller's
  ordinary **system** message maps to — Harmony reserves the actual system message
  for metadata (identity, cutoff, date, reasoning, channels). This is the single
  most common point of confusion for people writing the format by hand.
- **UX:** library only; requires a local model repo/template to be present.

### 4. Community ports (e.g. a multi-language fork adding JS/C# bindings)

- **Features:** same renderer surface re-exposed for JavaScript and C#.
- **Value:** shows demand for using the format outside Python/Rust — including from
  a browser — which is exactly the niche a client-side page fills.
- **Limits:** tracks upstream with lag; still a package install, not a tool.

### 5. Inference servers that hide the format (Ollama / vLLM / hosted providers)

- **Features:** accept ordinary chat messages and do Harmony rendering internally.
- **Positioning consequence:** the audience for a *renderer* is specifically people
  building their own inference loop, debugging a bad prompt, or writing tests — they
  need to **see** the exact string, not have it hidden. That argues for a page whose
  primary output is the literal rendered prompt, copy-able verbatim.

## Table-stakes list (what every serious reference exposes)

| Capability | Decision |
| --- | --- |
| Model identity line | **in-model** — `model_identity` param, default matches the library default |
| Knowledge cutoff line | **in-model** — `knowledge_cutoff`, default `2024-06` |
| Current date line | **in-model** — `current_date`, optional, date picker |
| Reasoning effort low/medium/high | **in-model** — `reasoning_effort` enum, default `medium`, plus a `none` choice to omit the line |
| Valid-channels line + "must be included" clause | **in-model** — always emitted with the system message |
| "Calls to these tools must go to the commentary channel" clause | **in-model** — emitted automatically when function tools are present |
| Developer instructions (`# Instructions`) | **in-model** — `instructions` param |
| Function tools as a TypeScript-ish `namespace functions { … }` block | **in-model** — `tools` param takes JSON Schema function defs and renders the namespace |
| system→developer mapping | **in-model** — a `system` role message in the input is folded into the developer Instructions, with the reason stated on the page |
| Whole-conversation vs render-for-completion | **in-model** — `render_target` enum (`conversation` / `completion`) |
| Assistant channels (`analysis` / `commentary` / `final`) | **in-model** — per-message `channel`, defaults to `final` |
| Tool calls (`to=functions.NAME`, `<\|constrain\|>json`, `<\|call\|>` stop) | **in-model** |
| Tool output messages (`functions.NAME to=assistant`) | **in-model** |
| Chain-of-thought drop rule | **in-model** — `auto_drop_analysis`, default on |
| Structured output alongside the prompt | **in-model** — `output_format` enum (`text` / `json` with counts + stop tokens) |
| Accepts a plain OpenAI-style JSON message array | **in-model** — `input_format` `json` |
| Accepts a quick `role: content` line format | **in-model** (our addition) — `input_format` `lines`, `auto` sniffs |

## UX control patterns adopted

- **Preset chips** for the recurring shapes the guides walk through: a minimal
  system+user prompt, a developer-instructions prompt, a tool-calling round trip,
  and a chain-of-thought drop demo. The reference material is example-driven, so
  one-click examples are the right port of that.
- **Select controls** for every fixed-choice knob (`input_format`,
  `reasoning_effort`, `render_target`, `output_format`) rather than free text.
- **Date picker** for `current_date`, since the format wants `YYYY-MM-DD`.
- **Multiline textareas** for the conversation and the tool JSON — both are pasted.
- **Copy-the-result** and **Reset** come from the shared page chrome.
- Output is the literal rendered prompt with the control tokens visible, because
  the whole point is to inspect the exact string.

## Out-of-model — considered, not built

- **Token IDs / token counts.** Requires shipping the `o200k_harmony` BPE
  vocabulary (megabytes) into the browser bundle. The FAQ documents the control
  tokens' IDs instead so a reader can splice them into their own tokenizer.
- **Built-in `browser` and `python` tool namespaces.** Their namespace text must be
  byte-identical to what the model was trained on; paraphrasing it (our no-copy
  rule) would produce a subtly wrong prompt, and reproducing it verbatim is
  copying. Declined on both grounds, and the page says so — users needing the
  built-in tools should render those with the official library.
- **Parsing model completions back into messages.** A separate direction (and a
  separate tool); this one renders.
- **Live inference / "try this prompt against a model".** Needs a server and a key.

## Known ambiguity, resolved and documented

The published examples show the assistant tool call as
`<|start|>assistant<|channel|>commentary to=functions.NAME <|constrain|>json<|message|>`
(channel first), while the library's header builder emits the recipient before the
channel marker. Both forms are accepted by the format's parser. This tool emits the
documented example form so output can be diffed against the published guides, and
the page's FAQ states the equivalence explicitly.
