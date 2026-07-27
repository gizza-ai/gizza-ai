## About this tool

This Persian/Farsi normalizer cleans text copied from mixed Arabic/Persian
sources. It can fold Arabic code points into Persian letters, convert digit
scripts, tidy نیم‌فاصله (ZWNJ) half-spaces, correct punctuation spacing, strip
harakat/diacritics, and collapse messy whitespace. Each pass is optional, so you
can run a full cleanup or isolate one rule while preserving the rest of the text.

Common fixes include:

- `ك` → `ک`, `ي` / `ى` → `ی`, and `ة` → `ه`.
- ASCII and Arabic-Indic digits → Persian digits, or the reverse to English.
- `مي روم` / `می روم` style prefixes and `کتاب ها` style suffixes → ZWNJ forms.
- No stray space before punctuation, and one readable space after punctuation.

### Worked example

Input:

```text
كتاب123 را مي خواهم ,خوب
```

With the default cleanup options, the output is:

```text
کتاب۱۲۳ را می‌خواهم, خوب
```

Turn on **Persian punctuation** if you also want Latin punctuation converted, for
example `,` to `،` and `?` to `؟`.

## FAQ

<details>
<summary>What is ZWNJ / نیم‌فاصله?</summary>

ZWNJ is the zero-width non-joiner character used as a Persian half-space. It keeps
letters visually separated without a full word space, as in `می‌روم` or `کتاب‌ها`.
The half-space option cleans spaces around existing ZWNJs and adds them for common
prefixes and suffixes.

</details>

<details>
<summary>Can I keep English digits?</summary>

Yes. Set **Digits** to **English digits** to convert Persian and Arabic-Indic
digits to `0-9`, or choose **Keep** to leave all digit scripts untouched. The
default is Persian digits because this tool is optimized for Persian publishing.

</details>

<details>
<summary>Does it change meaning or rewrite sentences?</summary>

No. It is a deterministic text normalizer, not a spell checker or rewriter. It
fixes Unicode variants, spacing, punctuation, digits, and optional diacritics;
it does not infer grammar, lemmatize words, or replace vocabulary.

</details>

<details>
<summary>Why is Persian punctuation off by default?</summary>

Changing `,`, `;`, and `?` to Persian `،`, `؛`, and `؟` is useful for final copy,
but it is more content-changing than spacing or Unicode folding. Enable it when
you want Persian punctuation marks; leave it off when preserving Latin punctuation
from source data matters.

</details>
