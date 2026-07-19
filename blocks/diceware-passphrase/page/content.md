## About this tool

Diceware is a way to build passwords you can actually remember: instead of a jumble of characters, you chain random common words — the classic "correct horse battery staple" idea. Each word is chosen by (real or simulated) dice rolls against a fixed wordlist, so every word adds a known, provable amount of randomness. This generator uses the EFF wordlists — the **EFF long list** (7,776 words, five dice per word, ~12.9 bits of entropy per word) and the **EFF short list** (1,296 shorter words, four dice per word, ~10.3 bits per word) — and draws words with your browser's cryptographic random number generator. Nothing is sent anywhere: the wordlists and the RNG run locally on your device.

Pick how many words you want (six is the widely recommended default — about 77.5 bits), choose a separator, and optionally capitalize the words or append a random digit or symbol for sites that demand mixed character classes. You can also generate a whole batch at once, or flip on **Show dice rolls** to see the five-digit roll behind every word.

Purists can do it the traditional way: roll physical six-sided dice and type the digits into the **Your own dice rolls** field (five digits per word for the long list, four for the short list). The tool then simply looks the words up — fully deterministic, no RNG involved.

### Worked example

Typing the dice rolls `62315 14534 23633 31662 35553 44151` with the EFF long list and a hyphen separator produces:

```
tiger-canal-dolphin-garlic-lantern-pebble

Entropy: 77.5 bits — strength: strong
Recipe: 6 words from the EFF long list (7,776 words, 12.9 bits/word)
Crack time: about 339 thousand years (offline, 10 billion guesses/sec)
```

Six random words feel almost silly to type — and would still take a modern offline cracking rig hundreds of thousands of years to brute-force on average.

### Limits and notes

- 2–20 words per passphrase, and 1–20 passphrases per batch.
- When you supply your own dice rolls, the batch count must be 1 (one set of rolls = one passphrase), and the roll digits must be 1–6 in groups of 5 (long list) or 4 (short list).
- Capitalization adds **no** entropy (an attacker can try both cases cheaply) — it's purely for readability or for sites that require an uppercase letter.
- The random digit/symbol extras add ~3.3 / ~3.6 bits each — a convenience for password rules, not a substitute for another word (~12.9 bits).
- Crack-time figures assume an offline attacker testing 10 billion guesses per second who knows you used this exact method — the guarantee comes from the word count, not from secrecy of the wordlist.
- Wordlists are English only.

## FAQ

<details>
<summary>How many words should my passphrase have?</summary>

Six words from the EFF long list (~77.5 bits) is the widely recommended baseline for anything important, and it's this tool's default. Five words (~64.6 bits) is acceptable for lower-value accounts; use eight or more (~103+ bits) for a password-manager master password, disk encryption, or cryptocurrency wallets. Each extra long-list word adds ~12.9 bits — roughly a 7,776× harder brute-force.

</details>

<details>
<summary>What's the difference between the EFF long and short wordlists?</summary>

The long list has 7,776 words (five dice per word, ~12.9 bits each) and includes words up to 9 letters. The short list has 1,296 words (four dice per word, ~10.3 bits each) that are at most 5 letters long — easier to type on phones, but you need more of them for the same strength: six short-list words ≈ 62 bits, about the same as five long-list words.

</details>

<details>
<summary>Is it safe to generate a passphrase in the browser instead of rolling real dice?</summary>

The words are drawn with the browser's cryptographic RNG (the same class of generator used for TLS keys), with rejection sampling so every word is exactly equally likely. That is more than adequate. If you prefer the traditional ceremony — or don't want to trust any software RNG — roll physical dice and type the digits into the "Your own dice rolls" field; the tool then only performs the wordlist lookup, entirely offline on your device.

</details>

<details>
<summary>Does capitalizing words or adding a digit make the passphrase stronger?</summary>

Capitalization adds nothing: it's deterministic, so an attacker's cost is unchanged. The "append a random digit" (+3.3 bits) and "append a random symbol" (+3.6 bits) options add a little real entropy because the character is randomly chosen — but a whole extra word adds ~12.9 bits, so prefer another word when strength is the goal. The extras mainly exist to satisfy password rules that demand digits or symbols.

</details>

<details>
<summary>Why does the strength report assume the attacker knows my method?</summary>

That's the honest (Kerckhoffs') assumption behind every diceware entropy figure: the attacker is assumed to have the exact wordlist and to know you picked N words. The entropy is then exactly N × log2(list size) — for six long-list words, 7,776⁶ ≈ 2.2 × 10²³ possibilities. Any "extra" security from the attacker not knowing your method is a bonus, never something to rely on.

</details>
