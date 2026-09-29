## About this tool

Use this converter when you need to inspect the numeric codes behind text, debug encoding problems, or rebuild text from a copied list of codes. It supports four scopes: Unicode scalar values, UTF-8 bytes, UTF-16 code units, and strict ASCII.

The base control chooses decimal, hexadecimal, binary, or octal output. Encoding can add common prefixes such as `0x`, `\x`, or `U+`, choose spaces/commas/newlines/no delimiter, and zero-pad to fixed widths. Decoding is intentionally tolerant: it ignores common separators and strips the same prefixes before checking ranges.

### Worked example

Input text:

```text
Hello
```

Default output:

```text
72 101 108 108 111
```

With `base=hex`, `scope=utf8-bytes`, `prefix=0x`, and comma delimiter, the same text becomes:

```text
0x48, 0x65, 0x6C, 0x6C, 0x6F
```

### Limits and edge cases

The tool validates what each scope can represent. ASCII rejects characters above 127, UTF-8 bytes must be 0–255 and decode to valid UTF-8, UTF-16 units must form valid surrogate pairs, and Unicode scalar values reject surrogate code points and anything above U+10FFFF. Inputs are capped at 200,000 codes per call.

## FAQ

<details>
<summary>What is the difference between Unicode scalar and UTF-8 bytes?</summary>

Unicode scalar mode reports one code point per character-like scalar value, so `😀` is one code: `128512` or `U+1F600`. UTF-8 byte mode reports the encoded bytes used to store that scalar in UTF-8, so `😀` becomes four bytes: `240 159 152 128`.

</details>

<details>
<summary>When should I use UTF-16 units?</summary>

Use UTF-16 units when matching JavaScript string indexing, Windows APIs, or data formats that expose UTF-16 code units. Characters above U+FFFF become surrogate pairs, for example `😀` becomes `55357 56832`.

</details>

<details>
<summary>Why does ASCII mode reject accented characters and emoji?</summary>

ASCII is a 7-bit character set with values 0 through 127 only. Accented letters, emoji, and most non-English text need Unicode scalar, UTF-8 byte, or UTF-16 unit mode instead.

</details>

<details>
<summary>Can I paste codes with prefixes or separators?</summary>

Yes. Decode mode accepts whitespace, commas, semicolons, colons, pipes, dashes, slashes, quotes, and common prefixes such as `0x`, `\x`, `\u`, `U+`, `0b`, and `0o`. For fixed-width byte forms such as `48656c6c6f`, it can split the run automatically.

</details>
