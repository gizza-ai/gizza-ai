## About this tool

Shannon entropy measures how unpredictable a sequence is from its observed symbol frequencies. If every symbol appears equally often, entropy is high; if one symbol dominates, entropy is low. This calculator reports that value as **entropy per symbol** plus the derived totals people usually need for quick checks: total information, maximum possible entropy for the observed alphabet, efficiency, redundancy, perplexity, and a ranked frequency table.

It is an order-0 frequency calculation. It does **not** understand grammar, keyboard walks, leaked passwords, Markov models, or how an attacker guesses secrets. Use it as a compact information-theory measurement, not as a password-strength guarantee.

### Worked example — `password`

With the default character basis, `password` has eight characters and seven distinct character values because `s` appears twice. The report shows:

```text
Entropy: 2.7500 bits per character
Total information: 22.0000 bits across 8 characters
Distinct characters: 7
Maximum entropy for this alphabet: log2(7) = 2.8074 bits per character
Efficiency: 97.9539% (redundancy 2.0461%)
Perplexity: 6.7272 equally likely characters
```

That number is the entropy of the observed string. It does not mean the word `password` is safe; dictionary knowledge makes it easy to guess.

### Symbol basis

- **Characters** counts Unicode scalar values. This is the usual choice for text keys and passwords.
- **Bytes** counts UTF-8 bytes and produces the familiar 0–8 bits per byte range for binary-like text.
- **Words** counts whitespace-separated tokens, useful for prose or passphrases.
- **Lines** treats each whole line as one symbol, useful for repeated log events or list values.

### Scope and units

The default scope scores the whole input once. Use **Each line** or **Each paragraph** to find which records are repetitive versus varied; the combined report is still printed after the per-part summary.

Bits are base-2 units (shannons). Nats use base *e*, dits use base 10, and trits use base 3. Perplexity stays the effective number of equally likely symbols, regardless of the unit selected.

### Limits and edge cases

- Input is capped at 1 MiB and 20,000 lines/paragraphs for split scopes.
- Precision is 0–10 decimal places; frequency rows are capped at 64.
- Whitespace filtering removes whitespace characters for character/byte mode and blank lines for line mode. Word mode already splits on whitespace.
- A single repeated symbol has 0 entropy but 100% efficiency by convention because the one-symbol distribution is trivially uniform.
- Binary files are better handled by byte-oriented tooling; this page measures pasted text.

## FAQ

<details>
<summary>Is this the same as password strength?</summary>

No. Shannon entropy here is computed only from the symbols in the pasted string. It does not know that `password`, `qwerty`, dates, or dictionary words are common guesses. A random-looking 12-character token and a memorized word can have similar frequency entropy but very different attack resistance. Treat this as a measurement of symbol distribution, not a security score.

</details>

<details>
<summary>When should I choose bytes instead of characters?</summary>

Choose **bytes** when you want the conventional 0–8 bits/byte view or when UTF-8 encoding itself matters. Non-ASCII characters occupy multiple bytes, so byte entropy can differ from character entropy. For human text, characters or words are usually easier to interpret.

</details>

<details>
<summary>What does maximum entropy mean?</summary>

Maximum entropy is the value the input would have if every distinct symbol you observed appeared equally often. For seven distinct characters in bits, it is `log2(7)`. Efficiency is the measured entropy divided by that maximum. High efficiency means the observed counts are close to uniform; it does not prove the data was generated randomly.

</details>

<details>
<summary>Why do two differently ordered strings get the same entropy?</summary>

This calculator uses an order-0 model: it counts how often symbols appear and ignores their order. `abababab` and `aabbaabb` therefore have the same entropy because both contain four `a` and four `b`. If you need sequence predictability, you need a model that considers n-grams or transitions.

</details>

<details>
<summary>What is perplexity?</summary>

Perplexity is `base^entropy`: the effective number of equally likely symbols. An entropy of 2 bits per character has perplexity 4, meaning the distribution behaves like four equally likely characters even if the real alphabet is larger.

</details>
