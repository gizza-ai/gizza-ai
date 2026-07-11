# context-trimmer — competitor analysis (2026-07-10)

Goal: trim or truncate long text to an approximate LLM context/token budget while preserving the part users care about. Findings are paraphrased; no competitor wording or branding is copied.

## Competitors / references scanned

1. **llm-context-trim (Python library)** — trims message lists to a token budget, preserving high-value sections such as system messages and recent turns. Main pattern: budget-aware trimming with protected head/tail content.
2. **Text truncation strategy guides for LLM tooling** — document the common truncation choices: keep the beginning, keep the end, keep both ends, or summarize/drop the middle. They emphasize avoiding context-window errors and being explicit about information loss.
3. **OpenAI/Codex token-limit discussions** — note that simple head+tail truncation is common for tool output, but can hide important middle context; users need strategy choice rather than a single hard-coded policy.
4. **Open WebUI context-window documentation** — highlights a key limitation: different models tokenize differently, so blind exact trimming is model-dependent. A transparent approximation is acceptable only when labelled as approximate.

## Table-stakes → decision

| Capability | In/Out of model | Decision |
| --- | --- | --- |
| Target token budget | in-model | `max_tokens` integer, default 512, bounded 1–1,000,000 |
| Token estimate knob | in-model | `chars_per_token` number, default 4.0, bounded 1–20 |
| Keep beginning | in-model | `keep=head` |
| Keep ending / latest text | in-model | `keep=tail` |
| Keep middle | in-model | `keep=middle` |
| Keep both beginning and ending | in-model | `keep=head_tail` plus `head_ratio` split |
| Omission marker | in-model | `marker`, default `…`, counted in the budget |
| Avoid split words | in-model | `break_words=false` default; toggle for exact hard cuts |
| Exact model tokenizer counts | out-of-model | listed as a limit; no model-specific tokenizer dependency |
| Summarization/semantic compression | out-of-model | needs an LLM; not part of this pure Rust tool |

## Our design

Pure Rust, browser-local, deterministic character-budget trimming. It returns unchanged text when the input already fits; otherwise it inserts the chosen marker where content was removed and keeps the result within the approximate token budget.
