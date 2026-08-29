## About this tool

Ethereum vanity addresses are normal secp256k1 keypairs whose derived `0x` address happens to start or end with memorable hexadecimal characters. This tool grinds candidate private keys locally, derives each Ethereum address with Keccak-256, and stops at the first address that matches your prefix, suffix, or combined pattern.

Use short patterns. Each case-insensitive hex character multiplies expected work by 16: `abc` averages about 4,096 keys, `dead` averages 65,536, and five characters averages about 1,048,576. Turn on case-sensitive matching only when you need exact EIP-55 checksum casing; each letter then adds another 2x factor. The estimate output mode reports difficulty and odds without generating any key material.

Worked example: choose prefix `dead`, set output format to **Estimate only**, and keep the default 100,000 attempts. The result explains that a four-character case-insensitive prefix is about 1 in 65,536 and gives the chance of finding at least one match inside the attempt budget.

Limits and edge cases: prefix and suffix accept only hex characters `0-9` and `a-f`; a leading `0x` is ignored on the prefix. The combined pattern cannot exceed the 40 hex characters in an Ethereum address. The local cap is 5,000,000 attempts to keep browser and CLI runs bounded. Leave the seed blank for platform CSPRNG randomness when making a real wallet; a human-readable seed is reproducible and therefore unsafe for funds if somebody can guess it. This tool does not query balances, sign transactions, manage mnemonics, encrypt keyfiles, or use GPUs.

## FAQ

<details>
<summary>Is this safe for a real wallet?</summary>

It can be, but only when the seed field is blank so the run starts from the platform's cryptographically secure random source. A seed such as a word, project name, or ticket number makes the result reproducible and should be treated as a demo or test vector, not as a key for funds.

</details>

<details>
<summary>Why do longer prefixes get slow so quickly?</summary>

Ethereum addresses are hexadecimal, so each extra case-insensitive character has a 1-in-16 chance of matching. Expected work grows as `16^n` for `n` fixed positions before any EIP-55 case requirement, which is why short patterns are practical and long vanity strings are not in a single-threaded browser tool.

</details>

<details>
<summary>What does case-sensitive matching mean here?</summary>

Ethereum address casing is the EIP-55 checksum. With case-sensitive matching off, `ab`, `AB`, and `Ab` all target the same lowercase hex prefix. With it on, letter casing must match the final checksummed address exactly, which reduces the hit rate for each letter.

</details>

<details>
<summary>Can I search for text outside hexadecimal characters?</summary>

No. Ethereum addresses are 40 hexadecimal characters after `0x`, so patterns can only use `0-9` and `a-f`. For a word-like result, spell it with hex-looking characters such as `cafe`, `babe`, `dead`, or `feed`.

</details>
