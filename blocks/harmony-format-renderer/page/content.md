## About this tool

The gpt-oss open-weight models do not read an ordinary chat message array. They read *Harmony* — a single flat string built from control tokens such as `<|start|>`, `<|channel|>`, `<|constrain|>`, `<|message|>`, `<|end|>`, `<|call|>` and `<|return|>`. If you are writing your own inference loop, debugging a prompt that the model answered strangely, or writing a golden-file test, you need to see that exact string. This renderer builds it from a conversation you paste, entirely in your browser, with no tokenizer, no model and no network request.

Paste the conversation either as a JSON array of `{role, content}` objects — the shape every chat API already uses, with optional `channel`, `recipient` and `name` fields — or as one turn per line in the shorter `role: content` form, where `assistant[analysis]: …` sets the channel and `assistant[commentary] to=get_weather: {…}` marks a tool call. **Conversation format** forces one reader or lets the tool sniff (a leading `[` or `{` means JSON). Content parts (`[{"type": "input_text", "text": "…"}]`) and the Chat Completions `{"type":"function","function":{…}}` tool wrapper are both flattened, so a copy-pasted API payload renders as-is.

The **system** message in Harmony is metadata, not your prompt: it carries the model identity line, `Knowledge cutoff:`, an optional `Current date:`, the `Reasoning:` level, and the valid-channels clause. Your actual prompt belongs in the **developer** message under `# Instructions`. That is the single most common mistake when hand-writing the format, so any `system` or `developer` turn found in the conversation is folded into the developer message automatically, appended after whatever you typed in **Developer instructions**.

Add **Function tools** as JSON Schema definitions and they render as the TypeScript-flavoured `namespace functions { … }` block the models were trained to read — each parameter typed, optional parameters marked with `?`, enums as unions, descriptions as `//` comments, defaults as trailing comments — and the system message gains the clause that routes tool calls to the commentary channel. **Render target** decides whether the output ends with the `<|start|>assistant` generation prompt (ready to sample from) or stops after the last turn. **Drop superseded analysis turns** applies Harmony's chain-of-thought rule: analysis that came before the last `final` answer is discarded, while analysis belonging to an in-flight tool-calling chain is kept. Switch **Output** to the JSON report to get the prompt plus the message, tool and character counts and the stop-token list.

**Worked example.** Choose the *Tool-calling round trip* example, or paste these five lines with the conversation format set to `role: content` lines, one `get_weather` tool declared, and the drop rule off:

```text
user: what is the weather in Oslo?
assistant[analysis]: the user wants the current weather
assistant[commentary] to=get_weather: {"city":"Oslo"}
tool:get_weather: {"c":21}
assistant: It is 21 C in Oslo.
```

The renderer emits the system metadata message, a developer message holding your instructions plus `namespace functions { … }`, then `<|start|>user<|message|>what is the weather in Oslo?<|end|>`, the analysis turn on `<|channel|>analysis`, the call as `<|start|>assistant<|channel|>commentary to=functions.get_weather <|constrain|>json<|message|>{"city":"Oslo"}<|call|>`, the result as `<|start|>functions.get_weather to=assistant<|channel|>commentary<|message|>{"c":21}<|end|>`, and the answer on `<|channel|>final`. Bare tool names are namespaced to `functions.NAME` for you; a name that already contains a dot is left alone.

**Limits.** The conversation is capped at 200,000 characters and 500 turns, the tool JSON at 50,000 characters. This is deterministic string assembly, so it emits text, never token IDs — reproducing the `o200k_harmony` vocabulary in a browser page is not practical. The built-in `browser` and `python` tool namespaces are not generated: their wording has to be byte-identical to the training data, so render those with the official library instead. Parsing a model's completion back into messages is the opposite direction and is not part of this tool.

## FAQ

<details>
<summary>Why did my system prompt end up in a developer message?</summary>

Because that is what the format requires. Harmony reserves the `system` message for metadata — identity, knowledge cutoff, current date, reasoning effort and the channel rules — and puts caller instructions in the `developer` message under an `# Instructions` heading. A `system` turn in your conversation is therefore folded into the developer message rather than emitted verbatim, which is the same mapping the packaged chat templates perform. If you want the metadata message suppressed entirely, uncheck **Include the system (metadata) message**.

</details>

<details>
<summary>What are the analysis, commentary and final channels?</summary>

`analysis` carries chain-of-thought, `commentary` carries tool calls and preambles, and `final` carries the answer meant for the user. Only `final` should ever be shown to an end user. An assistant turn with no channel is rendered as `final`, unless it has a `recipient`, in which case it is a tool call and renders on `commentary`.

</details>

<details>
<summary>Should I keep the old analysis turns in my history?</summary>

Usually not. Once the assistant has produced a `final` answer, the analysis that led to it is dropped from the history that gets fed back in — that is the behaviour of **Drop superseded analysis turns**, which is on by default. Analysis produced after the last `final` answer belongs to a tool-calling chain that is still running, so it is kept. Turn the option off when you want a verbatim render of exactly the turns you supplied, for example when writing a test fixture.

</details>

<details>
<summary>Does it output token IDs, and what are the stop tokens?</summary>

No — the output is text, and it stays text. Emitting IDs would mean shipping the whole `o200k_harmony` vocabulary into the page. When you sample, stop on `<|return|>` (the model finished its answer) and on `<|call|>` (the model wants a tool result); the JSON report lists both. When you append the completed turn back into history, the convention is to store it terminated with `<|end|>` rather than `<|return|>`.

</details>

<details>
<summary>The tool-call header looks different from another renderer I use.</summary>

The published examples write the assistant tool call as `<|channel|>commentary to=functions.NAME <|constrain|>json`, with the channel before the recipient, while some library code emits the recipient first. Both orderings are accepted by the format's parser. This tool follows the documented example form so its output can be diffed line by line against the published guides.

</details>

<details>
<summary>Is the conversation I paste sent anywhere?</summary>

No. The renderer is compiled to WebAssembly and runs entirely in your browser: the conversation, your instructions and your tool schemas never leave the page, and nothing is uploaded, logged or stored. It is safe to paste an internal prompt you are debugging.

</details>
