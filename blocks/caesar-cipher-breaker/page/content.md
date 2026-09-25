## About this tool

A Caesar cipher shifts every letter by the same amount. That makes it easy to encipher by hand, but also easy to break: there are only 26 possible shifts. This tool tries them all, scores each candidate against expected letter frequencies, and shows the most likely plaintext.

The original spacing, punctuation, case, and symbols are preserved. By default digits are left alone, because a classic Caesar cipher only shifts letters. If your puzzle also rotated digits, turn on **Also rotate digits**.

### Worked example

Paste this ciphertext:

```text
Wkh txlfn eurzq ira mxpsv ryhu wkh odcb grj.
```

The tool reports shift `3` and recovers:

```text
The quick brown fox jumps over the lazy dog.
```

Switch **Output view** to **Ranked candidates** to compare the top guesses, or **All 26 decryptions** when the text is very short and frequency analysis cannot confidently choose. **Diagnostic report** adds the index of coincidence and letter-frequency table for debugging puzzle text.

## Options and limits

- **Languages**: English, French, German, Spanish, Italian, and Portuguese frequency profiles.
- **Output views**: best, ranked, all shifts, or a frequency-analysis report.
- **Top candidates**: 1 through 26 rows in the ranked view.
- **Digit rotation**: optional, using `shift mod 10`.
- Limit: 20,000 characters per run. Only ASCII A-Z letters are scored, but every character is preserved in the plaintext.
- Very short inputs include a warning because letter frequency is noisy below about 20 letters.

## FAQ

<details>
<summary>What shift number does the tool report?</summary>

The shift is the amount that was originally applied to make the ciphertext. A reported shift of `3` means `A` was enciphered as `D`, and the tool decrypted by shifting letters back three places.

</details>

<details>
<summary>Why is the confidence low or wrong on a short word?</summary>

Frequency analysis needs enough letters to recognize the language. A single word like `Khoor` can still be cracked by inspection, but many shifts look plausible statistically. Use the ranked or all-shifts view for short messages.

</details>

<details>
<summary>Does this break Vigenere or substitution ciphers?</summary>

No. It is intentionally a Caesar/shift-cipher breaker. If the index of coincidence is closer to a flat random alphabet, the diagnostic report may hint that the text is not a single Caesar shift.

</details>

<details>
<summary>What happens to punctuation, case, accents, and numbers?</summary>

ASCII letters are shifted back while case is preserved. Spaces, punctuation, emoji, and symbols are copied unchanged. Accented letters are not scored or shifted. Digits are copied unchanged unless **Also rotate digits** is enabled.

</details>
