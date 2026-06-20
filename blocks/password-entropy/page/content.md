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
