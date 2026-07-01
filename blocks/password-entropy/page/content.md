## Check password strength in your browser

Type or paste a password to see its estimated **entropy in bits**, a strength
rating, a rough **crack-time** estimate, and any weaknesses. Everything runs
locally in your browser — **your password is never uploaded or stored**.

### How the estimate works

- **Entropy ≈ length × log2(alphabet size)** where the alphabet is the set of
  character types you used (lowercase 26 + uppercase 26 + digits 10 + symbols 33
  …). More types and more length = more bits.
- **Rating:** under 28 bits Very weak, 36 Weak, 60 Fair, 128 Strong, above that
  Very strong.
- **Crack time** assumes a fast offline attacker (~10 billion guesses/second).

### Weakness flags

- Shorter than 8 characters
- Uses only one type of character
- Is (or contains) a very common password
- A single repeated character, or a sequential run like `1234` / `abcd`

This is a **heuristic** estimate of guessability, not a guarantee — a long,
random passphrase from a password manager is always the safest choice.

## FAQ

<details>
<summary>Is it safe to type a real password in here?</summary>

The analysis runs entirely in your browser via WebAssembly — the password is
never uploaded, logged, or stored. That said, the cautious habit is to test a
*similar* password (same length and character mix) rather than the exact one
you use.

</details>

<details>
<summary>Why does "P@ssw0rd123" score better than it should?</summary>

The bit count uses the ideal `length × log2(alphabet)` model, which assumes
every character is random and independent — it can't tell a mangled dictionary
word from true randomness. That's exactly what the weakness flags are for: a
common-password or pattern warning means the real-world guessability is far
worse than the bits suggest.

</details>

<details>
<summary>What attack speed does the crack-time estimate assume?</summary>

An offline attacker making 10 billion (10^10) guesses per second, finding the
password after searching half the keyspace on average. Anything at or above
about 200 bits is simply reported as "centuries (effectively uncrackable)".

</details>

<details>
<summary>Do spaces, emoji, or accented characters help?</summary>

Yes — a space adds 1 to the alphabet-size estimate, and any non-ASCII
character (emoji, accents, CJK) adds a generous 100, on top of the 26 + 26 +
10 + 33 for lowercase, uppercase, digits, and symbols. Length still matters
more than any single exotic character.

</details>
