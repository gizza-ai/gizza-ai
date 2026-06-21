## Generate a secure random token in your browser

Create cryptographically random **tokens**, **API keys**, **session IDs**, and
other secrets with a configurable length and character set. Everything runs
locally in your browser with a cryptographic RNG — the generated value never
leaves your device or touches a server.

### Character sets

- **hex** — lowercase hexadecimal (`0-9a-f`), the default. 32 characters = 128
  bits, a common length for API keys and CSRF tokens.
- **hex-upper** — uppercase hexadecimal (`0-9A-F`).
- **base64url** — URL-safe base64 alphabet (`A-Za-z0-9-_`), the densest preset
  (6 bits per character) and safe to drop straight into a URL or header.
- **alphanumeric** — base62 (`A-Za-z0-9`).
- **alphabetic** — letters only (`A-Za-z`).
- **numeric** — digits only (`0-9`), handy for OTP-style codes.
- **safe** — alphanumeric with the easily-confused characters (`0`/`O`, `1`/`l`/`I`)
  removed, so a token can be read aloud or typed without ambiguity.

Set a **custom alphabet** to draw from your own characters instead of a preset
(duplicates are ignored). Use **count** to generate a whole batch at once.

The output shows each token plus its estimated **entropy in bits** (higher is
stronger) and the size of the alphabet used.

### Tips

- 128 bits of entropy (e.g. 32 hex characters, or 22 base64url characters) is a
  solid default for API keys and secrets.
- Each token is drawn uniformly with rejection sampling, so there is no
  modulo bias even for non-power-of-two alphabets.
- Re-run for a fresh value — nothing is stored.
