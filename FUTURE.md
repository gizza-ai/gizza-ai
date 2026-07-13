# gizza-ai — Future Improvements

Out-of-scope items from the v1 design and possible enhancements. Nothing here blocks v1 launch; each entry is a candidate for its own future spec.

## Conversation & state

- **Multi-thread history.** Sidebar with past conversations, rename, archive, search across threads. Backed by the same `impresspress/messages` block — schema already supports it.
- **Export / import conversations.** Download a thread as JSON or Markdown; re-import on another device.
- **Per-conversation system prompt.** Settings panel per thread for personality / role.
- **Conversation forking.** Branch from any message to explore alternative replies without losing the original.

## Skills

- **User-installable skills via UI.** Today skills are build-time embedded. Add a "Skills" admin page that fetches a `block.wasm` + `manifest.json` from a URL, validates, and registers at runtime. Requires moving from `include_bytes!` to a runtime registry.
- **Skill marketplace / registry browse.** UI to discover community-published skill blocks from the wafer-run registry.
- **Per-skill enable/disable.** Settings toggle to hide a tool from the agent without uninstalling.
- **Skill permissions prompt.** First-use confirmation for skills that touch network, files, or external loaders.
- **More built-in skills:**
  - `pdf` — extract text, render pages, fill forms (loader: `pdf-lib`).
  - `image-classify` / `image-ocr` (loader: `onnx-runtime`).
  - `transformers-js` skills (summarization, embedding, translation) — loader: `transformers-js`.
  - `markdown-export` — convert conversation to printable markdown.
  - `clipboard` — read/write OS clipboard (where browser allows).
  - `code-runner` — execute snippets in a sandboxed wasm interpreter (Python via Pyodide, JS via QuickJS).

## Loader types

- **`onnx-runtime`** — load and run ONNX models in-browser. Unlocks vision/audio skills.
- **`transformers-js`** — Hugging Face's transformers.js as a loader; share model cache across skills.
- **`webgpu-compute`** — direct WebGPU shader skills for image filters, particle effects, etc.
- **`pyodide`** — Python sandbox loader for `code-runner` and scientific skills.

## Models & inference

- **Per-skill model overrides.** Use a small fast model for trivial tool dispatch and a larger model for the user-facing reply.
- **Speculative decoding / model warmup.** Pre-warm the next likely model based on usage patterns.
- **BYOK remote model option.** Settings field for OpenAI/Anthropic API keys for users who want a remote model alongside local options. Strictly opt-in; the default stays local.
- **Model recommendation engine.** Detect device GPU/RAM and suggest the best-fitting model.

## RAG & memory

- **Local document upload + embedding.** Drop a PDF/TXT/MD file in once; embed it via a `transformers-js` skill; the agent can search it on later turns. Stored entirely in OPFS — no server.
- **Long-term memory.** A dedicated facts store the agent can read/write across conversations ("remember I prefer metric units").

## UX polish

- **PWA install banner + offline-first manifest.** "Install gizza-ai" on supported browsers; full offline use after first load.
- **Voice input.** Web Speech API for dictation into the composer.
- **Voice output.** Optional TTS of assistant replies (Web Speech API or a `speech` skill loader).
- **Keyboard shortcuts.** Cmd/Ctrl+Enter to send, Cmd/Ctrl+K to open settings, Esc to close drawer.
- **Theme customization.** User-pickable accent colors beyond light/dark.
- **Token + speed counters.** Show tokens/sec, total tokens, model size — useful for power users tuning model choice.
- **Tool-call timeline view.** Visualize a multi-round agent loop with timing per round.

## Distribution

- **Embeddable chat widget.** A `<script>` snippet site owners can drop on their own pages — same browser-local model, but configured with a custom system prompt and persona.
- **Custom domains for self-hosters.** Documentation + scripts for deploying gizza-ai under a different domain with custom branding.
- **Desktop wrapper.** Tauri or similar for an offline desktop app with native file dialogs.

## Quality, observability, safety

- **Local-only telemetry dashboard.** A `/stats` page showing counts of conversations, tools used, models tried — never leaves the browser.
- **Conversation safety filters.** Optional client-side moderation pass on user input before inference.
- **Skill output sanitization.** Centralized HTML/Markdown sanitization for tool results that get rendered.
- **Reproducibility mode.** Pin model version + skill versions in conversation metadata so a reload replays identically.

## Beyond browser-local (different product)

These would no longer be "all local" and so are not strictly future work for this repo, but worth noting:

- **Sync across devices.** End-to-end encrypted sync of conversations + settings via a small server.
- **Team / shared skills workspace.** Skills defined centrally, used by a team's gizza-ai instances.
- **Cloud LLM fallback.** When local model can't handle a request, fall back to a configured remote provider (clearly indicated in the UI).

The cloud-side AI agent platform is covered by `impresspress/IMPRESSPRESS_AI_PLAN.md` and is intentionally a separate product.
