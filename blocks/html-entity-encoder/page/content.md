## What this tool does

Paste text and get back the same text with special characters turned into
**HTML character entities**. Two independent choices control the result:

- **Encode which characters** (scope):
  - **Minimal** (default) — only the five characters that HTML and XML treat as
    special: `&`, `<`, `>`, `"`, and `'`. This is what you need to safely drop
    text into markup without it being parsed as tags.
  - **All non-ASCII** — the five above *plus* every character above plain ASCII
    (accents, currency symbols, dashes, emoji), giving pure-ASCII output that
    survives any encoding.
  - **Every named character** — the five above *plus* every character that has
    an HTML5 named entity.
- **Entity format** (how each character is written):
  - **Named** (default) — the readable HTML5 name where one exists (`&amp;`,
    `&copy;`, `&mdash;`), falling back to a decimal reference for characters that
    have no name.
  - **Decimal** — always a decimal numeric reference, e.g. `&#38;`, `&#169;`.
  - **Hex** — always a hexadecimal numeric reference, e.g. `&#x26;`, `&#xA9;`.

Everything runs locally in your browser with WebAssembly. Nothing is uploaded,
it works offline once loaded, and there is no sign-up.

## Worked example

**Input**

```
<a href="x">Tom & Jerry's</a>
```

**Output** (scope **Minimal**, format **Named**)

```
&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&apos;s&lt;/a&gt;
```

Only the five HTML-sensitive characters change; the letters, spaces, and slashes
are left exactly as they were. Switch the scope to **All non-ASCII** and an input
like `Café © €5` becomes `Caf&eacute; &copy; &euro;5` in named format, or
`Caf&#233; &#169; &#8364;5` in decimal.

## Choosing a scope

| Scope | Encodes | Use it when |
| --- | --- | --- |
| **Minimal** | `& < > " '` only | Inserting untrusted or literal text into HTML/XML so it renders as text, not markup. |
| **All non-ASCII** | the five + every char above U+007F | You need pure-ASCII output (email, legacy systems, `Content-Type` you can't trust). |
| **Every named character** | the five + every char with an HTML5 name | You want the most human-readable entities across the whole HTML5 named set. |

The five HTML/XML-sensitive characters are **always** encoded, whatever scope you
pick — so the output is never unsafe to embed.

## Choosing a format

- **Named** is the most readable and is what most hand-written HTML uses:
  `&copy;`, `&mdash;`, `&nbsp;`. Characters without a name (many symbols, all
  emoji) fall back to a decimal reference automatically.
- **Decimal** and **Hex** are universal: every character has a numeric code, so
  there is never a fallback. Hex matches the style used in CSS and many specs
  (`&#xA9;`). Both decimal and hex render identically in every browser.

## Details worth knowing

- **The apostrophe.** In **named** format `'` becomes `&apos;` (an HTML5 name);
  in **decimal**/**hex** it becomes `&#39;` / `&#x27;`, which older HTML4 parsers
  also accept. Pick a numeric format if you need maximum compatibility.
- **Canonical names.** Where a character has several aliases (`&amp;` and
  `&AMP;`), the tool always outputs the standard lowercase form.
- **Already-encoded text is re-encoded.** Running `&amp;` through again yields
  `&amp;amp;`, because the tool encodes the literal `&`. Encode raw text once,
  not text that already contains entities.
- **Round trips.** Any output here decodes back to the original with the
  companion **HTML Entity Decoder** tool.

## Common entities

| Character | Named | Decimal | Hex |
| --- | --- | --- | --- |
| `&` | `&amp;` | `&#38;` | `&#x26;` |
| `<` | `&lt;` | `&#60;` | `&#x3C;` |
| `>` | `&gt;` | `&#62;` | `&#x3E;` |
| `"` | `&quot;` | `&#34;` | `&#x22;` |
| `'` | `&apos;` | `&#39;` | `&#x27;` |
| `©` | `&copy;` | `&#169;` | `&#xA9;` |
| `—` | `&mdash;` | `&#8212;` | `&#x2014;` |
| `€` | `&euro;` | `&#8364;` | `&#x20AC;` |

## FAQ

<details>
<summary>Is my text uploaded anywhere?</summary>

No. Encoding happens entirely in your browser with WebAssembly — your input
never leaves your device, and the tool keeps working offline once the page has
loaded.

</details>

<details>
<summary>Which characters does "Minimal" encode, and why only those?</summary>

Minimal encodes exactly the five characters HTML and XML treat as special:
`&`, `<`, `>`, `"`, and `'`. Encoding just these makes any text safe to place
inside markup without it being parsed as a tag, attribute, or entity — while
leaving everything else readable.

</details>

<details>
<summary>What's the difference between named, decimal, and hex output?</summary>

They are three ways to write the same character. **Named** uses a keyword
(`&copy;`), **decimal** uses the Unicode code point in base 10 (`&#169;`), and
**hex** uses it in base 16 (`&#xA9;`). All three decode to the identical
character in every browser; named is the most readable, numeric formats never
need a fallback.

</details>

<details>
<summary>Why did an emoji become <code>&#128512;</code> instead of a name?</summary>

Emoji and many symbols have no HTML5 named entity, so in **named** format the
tool falls back to a decimal numeric reference (`&#128512;`). Choose **decimal**
or **hex** format if you want every encoded character written numerically.

</details>

<details>
<summary>Does it re-encode text that already contains entities?</summary>

Yes. The tool encodes the literal characters it sees, so a stray `&` in an
existing entity like <code>&amp;</code> gets re-encoded to <code>&amp;amp;</code>.
Encode raw, un-encoded text once — don't run already-encoded HTML through it.

</details>

<details>
<summary>How do I turn entities back into normal characters?</summary>

Use the companion **HTML Entity Decoder** tool, which reverses every format this
encoder produces — named, decimal, and hex references all decode back to the
original text.

</details>
