## About this tool

**Multilingual Stemmer** turns inflected words into Snowball stems for search, tagging,
keyword normalization and small corpus analysis. A stem is the reduced form used as an
index key: `running` becomes `run`, Spanish `corriendo` becomes `corr`, and German
`Häusern` becomes `haus`.

Choose the language that matches the text, then choose how you want the result:

- **Stemmed text** preserves punctuation, spacing and line breaks while replacing each word
  with its stem.
- **Unique stems** lists the vocabulary after stemming.
- **Form → stem mapping** shows how every distinct surface form was normalized.
- **Stem frequency table** counts each stem and the forms that collapsed into it.
- **JSON groups + stats** gives machine-readable stem groups for indexing pipelines.

Everything runs locally in WebAssembly. No text is uploaded.

### Worked example

Input:

```text
The runners were running quickly. Studies studied studying.
```

With language **English** and output **Stemmed text**, the result is:

```text
the runner were run quick. studi studi studi.
```

For a search index, switch to **JSON groups + stats** to get counts and surface forms per
stem. For an analyst checking a vocabulary cleanup, **Form → stem mapping** is usually the
most readable view.

### Languages

The tool uses Snowball stemming algorithms for Arabic, Danish, Dutch, English, Finnish,
French, German, Greek, Hungarian, Italian, Norwegian, Portuguese, Romanian, Russian,
Spanish, Swedish, Tamil and Turkish.

### Limits and edge cases

- Maximum input: **200,000 characters** per run.
- Stemming is language-specific. The wrong language still returns output, but the stems are
  not meaningful.
- Stems are **not lemmas** and may not be dictionary words: `studies → studi` is expected.
- The tokenizer treats Unicode letters and digits as words and keeps apostrophes inside
  contractions.
- Use **Minimum word length** to keep short abbreviations such as `AI`, `API` or product
  codes unchanged.

Also available from the gizza CLI and in chat.

## FAQ

<details>
<summary>Is stemming the same as lemmatization?</summary>

No. Stemming strips suffixes according to language rules and returns an index key, not a
dictionary word. `studies`, `studied` and `studying` all become `studi` in English. A
lemmatizer would try to return the dictionary lemma `study`, which usually requires a
larger language model or dictionary.

</details>

<details>
<summary>Which language should I choose for mixed-language text?</summary>

Choose the dominant language, or split the text first and run each language separately.
Snowball algorithms are language-specific: German suffix rules applied to Spanish text, or
English rules applied to Turkish text, can produce misleading stems even though the tool
will still run.

</details>

<details>
<summary>Why did capitalization change?</summary>

By default, words are lowercased before stemming because Snowball algorithms are defined
for lowercase input. This makes `Running` and `running` collapse to the same stem. Turn off
**Lowercase before stemming** only when case distinctions are part of the token you need to
preserve.

</details>

<details>
<summary>What output should I use for search indexing?</summary>

Use **JSON groups + stats** if a downstream program needs counts and source forms, or
**Unique stems** if you only need the vocabulary. Use **Stemmed text** when you want to
feed a normalized text stream into another simple text tool while keeping punctuation and
line breaks in place.

</details>

<details>
<summary>Will it handle a full book or a production corpus?</summary>

It is intended for pasted snippets, keyword lists and small batches, not full-corpus ETL.
The per-run cap is 200,000 characters so the browser and chat WebAssembly sandbox stay
responsive. For larger corpora, split the text into chunks or run a dedicated indexing
pipeline.

</details>
