## About this tool

Use this recovery code generator to create a printable set of one-time account-recovery codes for
2FA fallback, break-glass access, and emergency sign-in procedures. Pick the number of codes, the
grouping style, the alphabet, the separator, and whether the output should be a numbered sheet,
plain list, CSV, or JSON. The generator uses the browser or WASI cryptographic random source by
default, and the optional `seed_hex` field makes a sheet reproducible for tests and documented
fixtures.

Each code is drawn uniformly from the selected alphabet without modulo bias. The output reports the
entropy per code and for the whole sheet so you can see how much guessing resistance the selected
shape provides. If you operate the service that will accept these recovery codes, enable the
SHA-256 or salted SHA-256 digest option and store the digest instead of the visible code. For BIP39
wallet recovery phrases, use the dedicated BIP39 mnemonic tool instead; this page is for one-time
backup codes.

### Worked example

For a deterministic test sheet, use `count = 3`, `blocks = 2`, `chars_per_block = 4`, `charset =
numeric`, `separator = -`, `output = numbered`, and `seed_hex = 00112233445566778899aabbccddeeff`.
The result is a numbered list of three dashed 8-digit codes plus an entropy summary. In production,
leave `seed_hex` blank so the platform CSPRNG creates fresh secrets every run.

### Limits and edge cases

- `count` is capped at 50 codes, matching common account-recovery sheet sizes.
- `blocks` is capped at 6 and `chars_per_block` at 16, so a code can contain up to 96 characters
  before separators.
- `separator` may be blank and can contain up to three characters, but it cannot be a character from
  the chosen alphabet, whitespace, a comma, or a quote.
- `seed_hex` must contain 8-128 hex digits. A seeded sheet is reproducible, not magically secret;
  anyone with the seed can regenerate the same codes.
- The alphabets are punctuation-free on purpose because recovery codes are often printed, read
  aloud, or typed under stress.

## FAQ

<details>
<summary>Are these the same thing as BIP39 recovery phrases?</summary>

No. BIP39 phrases are wallet seed phrases with a checksum word and a specific wordlist. This tool
generates one-time account backup codes for login recovery, such as the codes a service gives you
when you enable two-factor authentication. Use a BIP39 mnemonic tool for wallet recovery phrases.

</details>

<details>
<summary>How many backup codes should I generate?</summary>

Most services issue 8-10 recovery codes, and 10 is the default here. Generate enough for your
expected emergency uses, store them in a password manager, encrypted file, or printed safe copy, and
replace the whole sheet if any code is exposed or used.

</details>

<details>
<summary>Should I store the visible codes or the SHA-256 digests?</summary>

If you are a user saving the sheet for yourself, store the visible codes securely because those are
what you will type during account recovery. If you run the service that verifies the codes, store a
digest instead of the visible code; the `sha256` and `sha256-salted` outputs provide that server-side
storage material.

</details>

<details>
<summary>When should I set a seed?</summary>

Set `seed_hex` only when you need an exactly reproducible sheet for tests, examples, or a documented
fixture. For real account recovery, leave the seed blank so the codes come from the platform
cryptographic random generator and differ every run.

</details>
