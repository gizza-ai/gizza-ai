## About this tool

Persian text uses a few conventions that generic whitespace tokenizers miss. The most important is the zero-width non-joiner (ZWNJ, نیم‌فاصله): words such as `می‌خوانیم`, `نمی‌شود` and `کتاب‌ها` look like two visible pieces, but they are normally one word for word counts, search indexing and NLP preprocessing. This tokenizer keeps those compounds together by default and can split them when you need morpheme-like parts.

Paste Persian or Farsi text, choose whether you want words, sentences or both, and select the output format. The tokenizer is deterministic and rule-based — no model download, no training data, and no upload. It also recognizes Persian punctuation (`،` `؛` `؟` `«` `»`), Persian and Arabic-Indic digits, URLs, emails, mentions, hashtags, and date/number separators.

### Worked example

Input:

```text
ما کتاب می‌خوانیم. حال شما چطور است؟ قیمت ۱٬۲۵۰ تومان است.
```

With the default **Words** mode, **One per line** format and punctuation set to **Separate**, the output is:

```text
ما
کتاب
می‌خوانیم
.
حال
شما
چطور
است
؟
قیمت
۱٬۲۵۰
تومان
است
.
```

Turn on **Split half-space compounds** and `می‌خوانیم` becomes two tokens: `می` and `خوانیم`. Switch **Punctuation** to **Remove** when you want only lexical words and numbers.

### What is handled

- ZWNJ and ZWJ joiners, with optional splitting at ZWNJ.
- Persian and Arabic punctuation, including `؟` and `۔` as sentence endings.
- ASCII, Arabic-Indic and Persian digits; separator-bearing numbers such as `۱۳۹۶/۰۶/۱۱`, `۳٫۵`, `۱٬۰۰۰` and `1,250.75` stay whole by default.
- URLs, email addresses, `@mentions` and `#hashtags` stay one token when **Keep entities** is on.
- Optional normalization folds Arabic keyboard forms (`ي`, `ك`, `ى`, `ة`) to Persian forms and strips harakat/tatweel.
- Newline handling for paragraphs, wrapped prose, subtitles and one-item-per-line lists.

### Limits and edge cases

- Maximum input length is **200,000 Unicode characters**.
- This is a tokenizer, not a stemmer, lemmatizer, POS tagger or named-entity recognizer.
- Half-space correction is not automatic: if the input is missing ZWNJ characters, the tool will not infer where they should be inserted.
- JSON output is compact by design so it can be copied into scripts without cleanup.
- Punctuation **Attach** mode is a whitespace split; use **Separate** or **Remove** for NLP-style token lists.

## FAQ

<details>
<summary>Why does `می‌خوانیم` stay one token by default?</summary>

The character between `می` and `خوانیم` is ZWNJ (U+200C), not a normal space. In Persian writing it marks a half-space inside one written word. For word counts, search indexing and most preprocessing, keeping that compound as one token is the least surprising default. Turn on **Split half-space compounds** only when you specifically want the pieces.

</details>

<details>
<summary>Does this normalize Arabic keyboard characters?</summary>

Yes, when **Normalize** is on (the default). Arabic `ي` and `ك` are folded to Persian `ی` and `ک`, `ى` becomes `ی`, `ة` becomes `ه`, Arabic-Indic digits are converted to Persian digits, and harakat plus kashida are stripped. Turn normalization off if you need to preserve the exact original characters.

</details>

<details>
<summary>How are sentences split?</summary>

Sentence mode treats `.`, `!`, `?`, Persian `؟`, Arabic full stop `۔`, reversed question mark `⸮` and ellipsis as sentence terminators when they are followed by whitespace or the end of the text. Periods inside numbers, URLs and email addresses are not treated as sentence boundaries.

</details>

<details>
<summary>Can I use it for word counts?</summary>

Yes. Use **Words** mode, **Punctuation: Remove**, leave **Split half-space compounds** off, and keep **Normalize** on. That gives a practical word-token list for counting, search indexing and simple Persian text statistics.

</details>

<details>
<summary>Is this the same as Hazm or Parsivar?</summary>

No. Hazm, Parsivar and similar NLP libraries provide larger pipelines such as normalization, stemming, lemmatization, POS tagging or parsing. This tool intentionally stays smaller: a deterministic in-browser tokenizer with the controls needed for copy-paste text cleanup and lightweight preprocessing.

</details>
