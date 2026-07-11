# json-redact — competitor analysis (2026-07-11)

Scan for JSON secret-redaction / masking tools. One WebSearch; top real competitors
skimmed. Paraphrased notes only — no competitor copy, branding, or trademarks reused.

## Competitors skimmed

1. **MaskJSON (maskjson.com)** — browser-only JSON masker. Auto-detects common
   sensitive *field names* (`password`, `token`, `email`, `ssn`, `credit_card`,
   `api_key`, and 30+ more). Replaces matched values with `***`, `[REDACTED]`,
   `null`, empty string, or a custom placeholder. 100% client-side; nothing uploaded.
2. **CleanMyPrompt — Redact API Keys** — detects and redacts *values* by vendor
   prefix: OpenAI (`sk-`), AWS (`AKIA`), Google (`AIza`), Stripe (`sk_live_`/`rk_live_`),
   GitHub (`ghp_`). Aimed at cleaning secrets out of text before pasting to an LLM.
3. **Markdown Redaction Tool (trymarkdownviewer.com)** — regex scan for emails, IPs,
   API tokens, credit cards, UUIDs; each match replaced with a labeled placeholder
   like `[REDACTED:email]`. Runs entirely in-browser.

Context motivating the tool: security reporting found 80k+ public JSON pastes on
JSONFormatter/CodeBeautify leaking real secrets — a client-side, structure-aware
redactor is the safe alternative.

## Table-stakes → our decision (all in-model unless noted)

| Capability | Competitor | Our decision |
|---|---|---|
| Detect by sensitive **key name** | MaskJSON | **In** — normalized substring/exact key match (`password`,`secret`,`api_key`,`token`,`private_key`,`authorization`,`email`,`ssn`… etc.) |
| Detect secret-looking **values** | CleanMyPrompt, Markdown | **In** — `detect_values` bool: JWT, AWS `AKIA`, OpenAI `sk-`, GitHub `gh?_`, Stripe `sk_/rk_/pk_live/test`, Google `AIza`, Slack `xox…`, PEM private-key blocks, emails |
| Replacement styles (`***`, `[REDACTED]`, `null`, empty, preserve-length) | MaskJSON | **In** — `style` enum: `redacted`\|`mask`\|`null`\|`empty`\|`preserve-length` |
| Custom placeholder text | MaskJSON | **In** — `placeholder` (used by `redacted` style, default `[REDACTED]`) |
| Add your own field names | MaskJSON | **In** — `extra_keys` comma-separated markers |
| Preserve JSON structure & key order | MaskJSON | **In** — serde_json `preserve_order`, re-serialize pretty |
| 100% local / no upload | all three | **In** — pure-Rust wasm, runs in browser/CLI/chat |
| Report of what/where redacted | (implicit) | **In** — chat/CLI return redacted count + JSON paths |

## Out-of-model (listed, not built)

- **Entropy-based generic secret detection** (flag any high-Shannon-entropy string):
  feasible in pure Rust but noisy/false-positive-prone without tuning; deferred in
  favor of high-signal vendor patterns + key-name matching. Noted as a page limit.
- **Interactive click-to-toggle per-field masking UI** — needs a bespoke JSON tree
  editor; our page is a single-shot transform.
- **Format-preserving encryption / reversible tokenization** — that's `pii-tokenize`'s
  job; json-redact is one-way masking.

## UX patterns adopted

- `style` rendered as a `<select>` (enum) with friendly `[input.labels]`.
- `detect_values` boolean checkbox (default on).
- `[[example]]` preset chips prefilling a realistic secrets-laden JSON payload.
- Multiline textarea for the JSON input; text output with Download + Copy.
