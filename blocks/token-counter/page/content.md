## About this tool

Large language models don't read characters or words — they read **tokens**, sub-word
chunks produced by a byte-pair-encoding (BPE) tokenizer. Every prompt you send and every
completion you get back is billed per token, and every model has a fixed **context window**
measured in tokens. Knowing the token count of a piece of text tells you what a request will
cost and whether it fits.

This token counter tokenizes your text with the model's real BPE encoding and reports the
exact token count, the character count, an estimated prompt (input) cost, the output price
for reference, and how much of the model's context window the text uses.

- **Exact counts for OpenAI models.** GPT-5.5, GPT-5, GPT-4.1, GPT-4.1 mini, GPT-4o, and
  GPT-4o mini use the `o200k_base` encoding; GPT-4 Turbo and GPT-3.5 Turbo use `cl100k_base`.
  Both are OpenAI's published tiktoken encodings, so the counts are exact.
- **Approximate counts for Claude & Gemini.** Anthropic's and Google's tokenizers are
  proprietary and aren't shipped as an in-browser library, so counts for Claude and Gemini
  are an `o200k_base` approximation — close, but treat them as an estimate (labeled *approx*).
- **Cost estimate.** The input cost is `tokens × the model's input price ÷ 1,000,000`. Prices
  are an embedded snapshot dated in the output; they change, so verify current pricing on the
  provider's own pricing page before relying on a number.
- **100% private.** Everything runs in your browser via WebAssembly — your text is never
  uploaded, and no network request is made.

## FAQ

<details>
<summary>How are tokens different from words or characters?</summary>

A token is a sub-word chunk chosen by the model's BPE tokenizer. Common words are often a
single token, but longer or rarer words, punctuation, whitespace, and non-English text can
split into several tokens. A rough rule of thumb for English is ~4 characters or ~0.75 words
per token, but this tool counts the **actual** tokens rather than estimating from length.

</details>

<details>
<summary>Are the token counts exact?</summary>

For OpenAI models, yes — they use OpenAI's published `o200k_base` and `cl100k_base` tiktoken
encodings, so the count matches what the API bills. For Claude and Gemini the count is an
approximation: those tokenizers are proprietary and aren't available as a browser library, so
we tokenize with `o200k_base` and label the result *approx*. It's a good ballpark, not an
exact bill.

</details>

<details>
<summary>How is the cost estimated?</summary>

Input cost = `tokens × the selected model's input price ÷ 1,000,000`. The tool shows the
per-1M input and output prices it used and the date of the embedded pricing snapshot. Model
prices change often, so always confirm the current rate on the provider's pricing page before
budgeting. The estimate covers the **input** (prompt) tokens only — the model's completion is
billed separately at the output rate shown.

</details>

<details>
<summary>Does this count the chat-format / message overhead?</summary>

No. It counts the plain encoding of the text you paste — the answer to "how many tokens is
this text". Chat APIs add a few tokens per message for role and formatting metadata, and that
overhead is provider- and endpoint-specific, so it isn't included here.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The tokenizer runs entirely in your browser as WebAssembly. Your text never leaves your
device and no network request is made, so it's safe to paste sensitive prompts.

</details>
