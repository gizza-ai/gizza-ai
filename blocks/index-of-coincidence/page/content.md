## What this tool does

The **Index of Coincidence (IC)** is the probability that two letters picked at
random from a text are the same. It is one of the oldest tools in cryptanalysis,
introduced by William F. Friedman in 1922. Because every language has a
characteristic letter distribution, the IC barely changes when you scramble a
text with a **monoalphabetic** cipher (a simple substitution or a transposition),
but it collapses toward the random value when you use a **polyalphabetic** cipher
like Vigenère. That single number tells you a lot about how a piece of text was
produced — all without knowing the key.

This calculator runs entirely in your browser. Nothing you paste is sent to a
server, it works offline once loaded, and there is no sign-up.

## How to read the result

The tool reports the IC two ways:

| Form | English plaintext | Uniform / random text |
| --- | --- | --- |
| **Normalized** (×26) | ≈ 1.73 | ≈ 1.00 |
| **Raw** (probability) | ≈ 0.0667 | ≈ 0.0385 |

A high IC (near 1.73 normalized) means the letter distribution is **uneven** —
the text is plaintext, or enciphered with a method that keeps the distribution,
such as a Caesar shift, a simple substitution, or a columnar transposition. A low
IC (near 1.0) means the distribution is **flat**, which points to a long-key
polyalphabetic cipher, a one-time pad, or already-random data.

## Estimating a Vigenère key length

Set **Estimate key length up to** to a number greater than 0 and the tool also
runs a period analysis. For each candidate key length `p`, it splits the text
into `p` columns (taking every `p`-th letter) and averages the IC of those
columns. When `p` equals the true key length, each column was enciphered with a
single Caesar shift, so its IC jumps back up toward the plaintext value. The
period with the **highest** average column IC is the likely key length — the same
idea Friedman used, and a companion to the Kasiski examination.

## Examples

| Input | Normalized IC | Reading |
| --- | --- | --- |
| A paragraph of English prose | ≈ 1.7–1.9 | Monoalphabetic / plaintext |
| The same text with a Caesar shift | ≈ 1.7–1.9 | Unchanged — substitution preserves IC |
| Vigenère with a 5-letter key | ≈ 1.0–1.2 | Polyalphabetic |
| Output of a one-time pad | ≈ 1.0 | Random |

## What gets counted

Only the 26 Latin letters **A–Z** are counted, and case is ignored — `A` and `a`
are the same letter. Digits, spaces, punctuation, and non-Latin characters are
skipped, following the classical cryptanalytic convention. Turn on **Show
per-letter frequency table** to see the count and percentage of each letter.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. Your text never leaves your device, and the page
keeps working offline once it has loaded.

</details>

<details>
<summary>Why two numbers?</summary>

The raw IC is the literal probability (≈ 0.0667 for
English). Many references quote the normalized form (raw × 26 ≈ 1.73) because it
is easy to compare against 1.0 for random text. Both describe the same thing.

</details>

<details>
<summary>How much text do I need?</summary>

A few hundred letters give a reliable IC; key-length
estimation wants more, ideally several times the key length per column.

</details>

<details>
<summary>Does it break the cipher?</summary>

No — the IC and the period estimate tell you the
*kind* of cipher and the likely key length. You still recover the key and
plaintext separately (e.g. with frequency analysis on each column).

</details>
