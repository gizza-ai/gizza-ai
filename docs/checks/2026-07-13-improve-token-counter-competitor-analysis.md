# token-counter — competitor analysis (2026-07-13)

Tool: **token-counter** — "Counts LLM tokens for pasted text and estimates prompt cost for chosen
model pricing." Pure-compute, in-browser (no server round-trip). Built with real BPE tokenization
via the wasm-safe `tiktoken-rs` crate (both the OpenAI `o200k_base` and `cl100k_base` encodings
instantiate under `wafer build` and `wasm-pack --target web` — verified with a spike).

## Competitors surveyed

1. **Tiktokenizer / OpenAI Tokenizer** (tiktokenizer.vercel.app, platform.openai.com/tokenizer) —
   the reference. Runs tiktoken (cl100k_base, o200k_base, p50k/r50k, gpt2) in the browser; shows
   exact token count, character count, and a colorized token breakdown. No pricing.
2. **token-calculator.net** — model dropdown across OpenAI / Claude / Gemini; token count + input,
   output, and total estimated cost per model; multi-provider comparison table. States OpenAI counts
   are exact (tiktoken) and non-OpenAI counts are approximations.
3. **pricepertoken.com/token-counter** — paste text, pick model, get token count + prompt cost;
   large per-provider pricing tables kept current; "verify on provider site" disclaimer.
4. **Spoold LLM Token Calculator** — cl100k / o200k / "approximate" encodings toggle; token count,
   cost estimate, and context-window utilisation bar.
5. **llm-tokenizer.com / BenchLM / Toolsana** — token counter + GPT-4 / Claude / Gemini cost
   estimator; 100% in-browser, privacy-forward ("no text leaves your browser").

## Table-stakes params / features and how ours maps them

| Capability (competitor) | In model? | Our decision |
| --- | --- | --- |
| Paste arbitrary text, count tokens | ✅ in | `text` param (required, multiline textarea). |
| Pick a model → tokenizer + pricing | ✅ in | `model` enum (13 current models across OpenAI / Anthropic / Google), friendly `<select>` labels, preset chips. |
| Exact counts for OpenAI (tiktoken) | ✅ in | Real BPE via `tiktoken-rs`: GPT-5/4.1/4o → `o200k_base`; GPT-4-turbo/3.5 → `cl100k_base`. Output labels these **(exact)**. |
| Claude / Gemini token counts | ⚠️ partial | Claude/Gemini use proprietary tokenizers not publicly shipped as a wasm-safe crate; we approximate via `o200k_base` and label the result **(approx)** honestly on-page and in output. |
| Character count | ✅ in | Reported. |
| Estimated prompt (input) cost | ✅ in | `tokens × input-price / 1M`, with the arithmetic shown. |
| Output price reference | ✅ in | Output $/1M shown for the chosen model. |
| Context-window utilisation | ✅ in | Context window + `% used` shown per model. |
| 100% in-browser / privacy | ✅ in | Pure wasm; no network. Stated on page. |
| Colorized per-token breakdown | ❌ out | UI-heavy token-highlight view; the page renders a text summary. Listed, not built. |
| Live-current pricing feed | ❌ out | Prices are a dated snapshot (2026-07-13) embedded in the block; page + output say "estimate — verify on the provider's pricing page." A pure offline tool can't fetch live prices. |
| ChatML / chat-message framing overhead | ❌ out | We count the ordinary encoding of the pasted text (matches "how many tokens is this text"); per-message role/format overhead is provider-specific and omitted (noted on page). |

## Pricing snapshot embedded (per 1M tokens, input / output; 2026-07-13)

OpenAI (exact tokenizer): GPT-5.5 $5/$30, GPT-5 $1.25/$10, GPT-4.1 $2/$8, GPT-4.1 mini $0.40/$1.60,
GPT-4o $2.50/$10, GPT-4o mini $0.15/$0.60, GPT-4 Turbo $10/$30 (cl100k), GPT-3.5 Turbo $0.50/$1.50
(cl100k). Anthropic (approx): Claude Opus 4.8 $5/$25, Claude Sonnet 5 $3/$15, Claude Haiku 4.5 $1/$5.
Google (approx): Gemini 3 Pro $2/$12, Gemini 2.5 Flash $0.30/$2.50. Anthropic figures from the
claude-api skill's cached model table (2026-06-24); OpenAI/Google from provider pricing pages.

**No competitor copy, branding, or trademarks were copied.** Model names are factual identifiers.
Every table-stake above is either in the descriptor or explicitly listed as out-of-model.

Sources: platform.openai.com/tokenizer, tiktokenizer.vercel.app, token-calculator.net,
pricepertoken.com/token-counter, developers.openai.com/api/docs/pricing, ai.google.dev/gemini-api/docs/pricing.
