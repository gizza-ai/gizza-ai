## About this tool

`accent-stripper` converts Unicode text into ASCII-friendly text. It is for the everyday cleanup jobs where accents and non-ASCII letters break matching, filenames, URLs, search keys or legacy systems: `café` becomes `cafe`, `Straße` becomes `Strasse`, `Ærøskøbing` becomes `AEroskobing`, and `Москва` becomes `Moskva`.

The default **Transliterate to ASCII** mode uses a transliteration table, not just Unicode decomposition. That matters for letters such as `ß`, `ø`, `Æ`, `Ł`, `Đ`, Greek, Cyrillic and CJK characters, because many of them do not carry a removable combining accent. For a conservative normalization pass, switch to **Marks only**: it removes combining marks (`é` → `e`) but leaves letters like `ß` or `Ж` alone unless your unmapped policy removes or replaces them.

### Worked example

Input:

```
Crème Brûlée à la Française — Straße, Ærøskøbing, Москва
```

Default output:

```
Creme Brulee a la Francaise -- Strasse, AEroskobing, Moskva
```

For slug preparation, enable lowercase and collapse whitespace:

```
  Déjà Vu: Señor Piñata!  
```

becomes:

```
deja vu: senor pinata!
```

You can then pass that through a slugifier or punctuation filter if you need dashes instead of spaces.

### Options and limits

- **Conversion mode** controls the core behavior. `transliterate` aims for readable ASCII equivalents; `marks-only` removes combining marks and is deliberately narrower.
- **Characters still non-ASCII** chooses what happens after conversion: keep them, remove them, or replace each one with the replacement text.
- **Replacement text** must be ASCII and at most 8 characters. It only applies when the unmapped policy is `replace`.
- **Keep these characters** protects literal non-ASCII characters from conversion. Use this when a language-specific letter such as `ñ` must remain distinct.
- **Lowercase result** runs after conversion, so transliterated uppercase letters become lowercase too.
- **Collapse whitespace** trims each line and squeezes spaces/tabs to one plain space while preserving line breaks.
- **Return JSON audit report** returns the converted text plus counts of input characters, output characters, converted/kept/unmapped characters, and whether the final output is pure ASCII.
- The input limit is 200,000 characters per run. Split larger documents.
- This tool does not translate words between languages. It approximates characters, so names and addresses remain recognizable but not linguistically perfect.

## FAQ

<details>
<summary>Does this only remove accents, or does it transliterate too?</summary>

The default mode transliterates. It handles plain accents (`é` → `e`) and characters that do not have removable marks (`ß` → `ss`, `ø` → `o`, `Æ` → `AE`, `Ж` → `Zh`). Switch to **Marks only** if you want the narrower Unicode-decomposition behavior that only drops combining accents.

</details>

<details>
<summary>Why did some characters stay non-ASCII?</summary>

Either you used **Marks only**, you protected them with **Keep these characters**, or the transliteration table had no ASCII spelling for them. Set **Characters still non-ASCII** to `remove` or `replace` when the output must be strict ASCII, or enable the JSON audit report to see how many characters were unmapped.

</details>

<details>
<summary>Can I keep ñ while stripping other accents?</summary>

Yes. Put `ñ` in **Keep these characters**. Then `mañana café` becomes `mañana cafe`: the protected `ñ` stays, while the accented `é` is converted. The keep list is literal, so add each character you want protected.

</details>

<details>
<summary>Is this a full slug generator?</summary>

No. It prepares text for a slug by removing accents, optional lowercasing, and optional whitespace collapse. It intentionally does not remove punctuation, choose separators, enforce uniqueness, or apply site-specific URL rules. Chain it with a slug or regex cleanup tool when you need that final formatting.

</details>

<details>
<summary>Is my text uploaded?</summary>

No. The same Rust core runs locally in the WebAssembly page and in the CLI. The text is processed in your browser or terminal, not sent to a server.

</details>
