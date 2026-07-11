## About this tool

**Context Trimmer** shortens text so it fits an approximate LLM token budget. It is useful when you need to paste a long log, transcript, prompt, article, or chat-history excerpt into a model with a fixed context window.

The tool uses a simple, transparent estimate: `tokens ≈ characters ÷ characters per token`. The default is `4.0` characters per token, a common English rule of thumb. Lower the value for code or non-English text when you want a more conservative trim.

### Worked example

Input:

```text
the quick brown fox jumps over the lazy dog
```

Settings: `max_tokens = 5`, `chars_per_token = 4.0`, `keep = head_tail`, marker `…`.

Output:

```text
the quick…lazy dog
```

The marker counts toward the budget, so the final result still fits the target estimate.

### Keep strategies

- **Head** keeps the beginning and drops the end.
- **Tail** keeps the end and drops the beginning.
- **Middle** keeps the centre and drops both ends.
- **Head + tail** keeps the beginning and the end, dropping the middle. Use **Head ratio** to split the budget between the two sides.

### Limits

- This is an approximate character-based budget, not a model-specific tokenizer. Exact token counts vary by model and language.
- By default, cuts move to whitespace so words are not split. Turn on **Allow cutting inside words** for exact character-limit cuts.
- If the input already fits the budget, it is returned unchanged and no marker is inserted.
- The maximum budget is capped at 1,000,000 tokens to avoid accidental huge outputs.

## FAQ

<details>
<summary>Does this use the same tokenizer as GPT, Claude, or Llama?</summary>

No. It intentionally uses a transparent approximation: characters divided by the **Characters per token** value. Use a lower value for more conservative trimming when exact model-token fit matters.

</details>

<details>
<summary>Which keep mode should I choose for logs or stack traces?</summary>

Use **Head + tail** for logs, stack traces, and command output. The beginning often has setup context, while the end usually contains the failure. Use **Tail** when only the latest messages matter.

</details>

<details>
<summary>Why does the output sometimes use fewer tokens than the target?</summary>

When word-safe trimming is enabled, the cut backs up or moves forward to a whitespace boundary so it does not split words. That can leave a little unused budget. Turn on **Allow cutting inside words** for stricter character use.

</details>

<details>
<summary>Is the text uploaded anywhere?</summary>

No. The trim runs locally in your browser with WebAssembly; your text stays on your device.

</details>
