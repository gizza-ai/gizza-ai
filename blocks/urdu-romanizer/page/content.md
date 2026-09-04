## About this tool

**Urdu Romanizer** transliterates Urdu script into Roman (Latin) Urdu. It is a deterministic,
local converter: paste Urdu text, choose a romanization scheme, and get output without sending the
text to a server.

The default **Informal ASCII** scheme aims at everyday Roman Urdu: `ہے` becomes `hai`, Urdu digits
become `0-9`, and Urdu punctuation becomes Latin punctuation. The **ALA-LC** and **ISO 15919**
schemes keep more letter distinctions with diacritics, which is useful when you need to show the
underlying Urdu letters rather than only a casual pronunciation.

Urdu normally omits short vowels. This tool is honest about that: it can honour vowel marks when
they are present, insert a simple default `a` between consonants for readability, or omit short
vowels entirely. A small common-word list handles frequent informal spellings such as `ہے` → `hai`
and `پاکستان` → `pakistan`.

### Worked example

Input:

```text
یہ کتاب اچھی ہے۔
پاکستان ۲۰۲۶
```

Default output:

```text
Yeh kitab achhi hai.
Pakistan 2026
```

Switch **Digits** or **Punctuation** to “keep” when you need the original Urdu characters preserved,
or turn off **Use common-word spellings** to see the purely letter-by-letter result.

### Options

| Control | What it changes |
| --- | --- |
| Romanization scheme | Informal ASCII, ALA-LC, or ISO 15919. |
| Short vowels | Insert a default `a`, use only typed marks, or omit short vowels. |
| Use common-word spellings | Applies a small deterministic word list before letter-level romanization. |
| Digits | Converts Urdu / Arabic-Indic digits to ASCII, or keeps them. |
| Punctuation | Converts `۔ ، ؟ ؛ ٪` to `. , ? ; %`, or keeps them. |
| Capitalization | Lowercase/no change, sentence case, or title case. |

### Limits and edge cases

- This is transliteration, not translation. `کتاب` becomes an approximate Roman spelling, not an
  English word.
- Unmarked short vowels are ambiguous. Without vowel marks or a dictionary entry, `کتاب` is rendered
  mechanically as `katab`; a human may prefer `kitab`.
- Informal ASCII output is lossy: multiple Urdu letters collapse to the same Latin letters.
  Scholarly schemes preserve more distinctions but use combining marks and diacritics.
- Existing Latin text, emoji and line breaks pass through unchanged.
- The tool does not perform neural correction, grammar correction, reverse Roman-to-Urdu conversion,
  or server-side batch/file processing.

## FAQ

<details>
<summary>Why does the output sometimes miss the vowel I expect?</summary>

Urdu script usually does not write short vowels. A deterministic local converter cannot infer every
missing `a`, `i` or `u` the way a fluent reader can. If your text includes vowel marks such as zabar,
zer or pesh, they are honoured; otherwise the selected short-vowel policy and the common-word list
are the only hints available.

</details>

<details>
<summary>Which scheme should I choose?</summary>

Choose **Informal ASCII** for everyday Roman Urdu in messages, notes and social posts. Choose
**ALA-LC** or **ISO 15919** when you want to preserve distinctions between Urdu letters such as
`ت/ط`, `س/ص/ث` or `ز/ذ/ض/ظ`. Those scholarly modes are more precise, but they produce diacritics
that casual Roman Urdu writers usually do not type.

</details>

<details>
<summary>Does this use an AI model or upload my text?</summary>

No. It is a local WebAssembly transliterator with fixed tables and a small built-in common-word
list. That makes it private and repeatable, but it also means it will not do neural short-vowel
restoration, word-sense disambiguation or grammar correction.

</details>

<details>
<summary>Can this convert Roman Urdu back into Urdu script?</summary>

No. Reverse conversion is a different and much more ambiguous task because informal Roman Urdu is
not standardized or reversible. For example, several Urdu letters may all be typed as `z`, `s` or
`t` in ASCII. This tool only handles Urdu script to Roman output.

</details>
