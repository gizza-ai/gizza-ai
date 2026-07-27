## About this tool

**Chunk Text** splits a long document into smaller, overlapping pieces so you can
embed them for retrieval-augmented generation (RAG), semantic search, or any
pipeline that feeds context to a language model. Each chunk is a contiguous slice
of your original text, and every record carries its `start`/`end` character
offsets so downstream code can cite the exact span.

Choose how the size is measured — **tokens** (estimated from characters, since no
real BPE tokenizer runs in the browser), **characters**, or **words** — and how
much each chunk overlaps the one before it. Overlap keeps a sentence that
straddles a boundary from being lost when it is retrieved.

Pick where a chunk is allowed to end with **Split on**: `word` never cuts a word
in half, `sentence` ends on `.`/`!`/`?`, `paragraph` ends on blank lines, and
`character` is a hard cut at the exact size. Coarser boundaries fall back to finer
ones when a single unit is bigger than the chunk size — the same recursive
behaviour as LangChain's `RecursiveCharacterTextSplitter`.

Output as a pretty **JSON** array (with `id`, `text`, `chars`, `tokens`, `start`,
`end`), newline-delimited **JSONL** (one object per line, ready to stream into a
vector store), or **plain** text with the chunks separated by a `---` divider.
Everything runs locally in your browser — your document is never uploaded.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>What chunk size and overlap should I use?</summary>

A common starting point for RAG is **around 500 tokens per chunk with ~10%
overlap** (so 50 tokens), which is why those are the defaults here. Smaller chunks
(128–256 tokens) give more precise retrieval but fragment context; larger chunks
(800–1000 tokens) keep more context but dilute the embedding. Tune to your
embedding model's context window and your documents.

</details>

<details>
<summary>Are the token counts exact?</summary>

No — token counts are an **estimate** based on the *chars per token* setting
(default 4.0, a common English GPT approximation). A real byte-pair tokenizer does
not run in the browser wasm sandbox, so the number is a fast heuristic, not a
per-model BPE count. If you need exact tokens for a specific model, use a
dedicated token-counter tool; for splitting documents into embed-sized pieces the
estimate is usually close enough.

</details>

<details>
<summary>What does "overlap" actually do?</summary>

Overlap makes each chunk repeat the last few units of the previous chunk. If an
important sentence falls right on a chunk boundary, overlap ensures it appears
*whole* in at least one chunk, so a retriever can still surface it. Overlap is
measured in the same unit as the chunk size and must be **less than** the chunk
size.

</details>

<details>
<summary>How is this different from a plain text splitter?</summary>

A plain splitter cuts on a single delimiter. This tool produces **overlapping,
size-bounded** chunks and never splits a word (or sentence/paragraph) unless a
single unit is larger than the whole chunk — then it recursively falls back to a
finer boundary. That sliding-window-with-overlap behaviour is what RAG and
embedding pipelines expect.

</details>

<details>
<summary>What do the start and end fields mean?</summary>

`start` and `end` are **character offsets** into your original text (0-based, end
exclusive), so `text.slice(start, end)` in JavaScript — or `text[start:end]` in
Python — reproduces the chunk. They let you highlight, cite, or link back to the
exact source span a retrieved chunk came from.

</details>
