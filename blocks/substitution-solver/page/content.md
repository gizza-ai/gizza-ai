## About this tool

This substitution solver helps with monoalphabetic cryptograms: puzzles where each cipher letter always stands for one plaintext letter. Use **Solve automatically** to hill-climb a key from English letter, bigram, trigram, and word statistics, **Analyze frequencies** to work by hand, or **Decode with key** when you already know the 26-letter cipher-to-plain alphabet.

Worked example:

- Text: `Gsv jfrxp yildm ulc.`
- Mode: `decode`
- Key: `zyxwvutsrqponmlkjihgfedcba`
- Output begins with `Decoded with the key you supplied` and shows plaintext `The quick brown fox.`

Cribs lock known mappings before solving. For example, `G=t, S=h, V=e` says cipher `GSV` should decode as `the`. Word cribs such as `QVW=the` expand to individual letter locks.

## Limits and edge cases

- Designed for English monoalphabetic substitution ciphers. It is not a Vigenère, Playfair, homophonic, transposition, or modern-encryption breaker.
- Automatic solving is heuristic. Short texts, missing spaces, names, unusual vocabulary, or fewer than about 20 distinct cipher letters may need cribs or manual frequency analysis.
- Input is capped at 100,000 characters. The solver searches the first 1,200 letters for speed, then applies the best key to the full text.
- Only ASCII A-Z letters are substituted. Punctuation, digits, spacing, and case pass through when layout preservation is enabled.

## FAQ

<details>
<summary>What key format does decode mode expect?</summary>

Use 26 letters in cipher-letter order A-Z, where each position tells which plaintext letter that cipher letter represents. The Atbash key is `zyxwvutsrqponmlkjihgfedcba`: cipher A becomes z, B becomes y, and so on. Use `?` for cipher letters you have not solved yet.

</details>

<details>
<summary>Why is the automatic solve sometimes imperfect?</summary>

A substitution cipher has 26! possible alphabets, so the automatic mode uses deterministic hill-climbing instead of an exhaustive search. It works best on normal English prose with enough letters and word spacing. If the result looks close but wrong, raise effort or add cribs for words you recognize.

</details>

<details>
<summary>What do cribs do?</summary>

Cribs are known plaintext hints. Write `X=e` for a single letter or `QVW=the` for a word. The solver locks those mappings before searching, which can steer short or ambiguous cryptograms toward the intended plaintext.

</details>

<details>
<summary>How should I use the frequency analysis mode?</summary>

Analyze mode lists letter frequencies, repeated bigrams, index of coincidence, and a frequency-matched starting key. Use it to spot likely vowels and common pairs, then copy the proposed key into decode mode and replace uncertain letters with your own guesses.

</details>
