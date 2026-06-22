## What this tool does

When text that was encoded in one charset gets read as if it were another, you
get **mojibake** — garbled sequences like `cafÃ©` (should be `café`) or `â€œ`
(should be a curly quote `“`). This tool **re-decodes** that text: it reinterprets
the input's bytes under the legacy charset you choose and emits clean **UTF-8**.

Everything runs locally in your browser — nothing is uploaded, it works offline
once loaded, and there's no sign-up.

## How to use it

1. Paste the garbled text into **Garbled text**.
2. Leave **Source charset** on `auto` to let the tool try the common charsets
   and keep the cleanest result — or set it explicitly to the encoding the text
   was wrongly decoded as. The most common culprit is `windows-1252`.
3. Choose how to handle bytes that don't fit the charset under **On undecodable
   bytes**.
4. If the text was mangled more than once (double mojibake), raise **Repair
   passes** to peel off each layer.

## Auto-detect

Set **Source charset** to `auto` (the default) and the tool re-decodes the text
under each common legacy charset, scoring each result, and returns the cleanest
one — the one with the fewest replacement characters and stray control codes. If
nothing looks like a clean repair (for example, the text wasn't actually
mojibake), it tells you so; pick a charset explicitly to force a specific
re-decode.

## Source charset — what to pick

The charset field accepts standard (WHATWG) labels, case-insensitive, with the
usual aliases:

| You might see… | Try this charset |
| --- | --- |
| `Ã©`, `Ã¨`, `Ã¼`, `â€œ`, `â€™` | `windows-1252` |
| Western-European accents off by one | `iso-8859-1` (alias `latin1`) |
| Euro sign issues | `iso-8859-15` |
| Garbled Cyrillic | `windows-1251` or `koi8-r` |
| Garbled Japanese | `shift_jis` (alias `sjis`) or `euc-jp` |
| Garbled Korean | `euc-kr` |
| Garbled Simplified / Traditional Chinese | `gbk` / `big5` |

## On undecodable bytes

| Option | What it does |
| --- | --- |
| **replace** (default) | Substitutes the Unicode replacement character `�` (U+FFFD) for any byte sequence that isn't valid in the chosen charset, and keeps going. |
| **strict** | Stops and reports an error on the first undecodable byte, so you know the charset is wrong. |

## Examples

| Garbled input | Source charset | Clean output |
| --- | --- | --- |
| `cafÃ©` | `auto` | `café` |
| `â€œhiâ€` | `windows-1252` | `“hi”` |
| `naÃ¯ve rÃ©sumÃ©` | `windows-1252` | `naïve résumé` |

## FAQ

**What is mojibake?** It's garbled text that appears when bytes encoded in one
character set are decoded with a different one. The fix is to decode with the
*correct* charset — which is exactly what this tool does.

**Which charset should I choose?** Start with `windows-1252` for European text —
it's behind the vast majority of `Ã`-prefixed mojibake. If the result still looks
wrong, try `iso-8859-1`, and for non-Latin scripts pick the matching regional
encoding from the table above.

**Is it free and private?** Yes — your text never leaves your device, and the
tool keeps working offline once the page has loaded.

**Why didn't anything change?** If your text was already clean UTF-8 (or pure
ASCII), re-decoding it as a single-byte charset won't help — this tool only fixes
text that was genuinely mis-decoded.
