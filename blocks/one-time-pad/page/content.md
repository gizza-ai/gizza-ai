## Encrypt with a pad that is never repeated

A one-time pad is simple but strict: the pad must be random, at least as long as the message, and used once. This tool enforces the length rule instead of repeating a short key like a Vigenère or repeating-XOR cipher would. If the pad is too short, it stops with an error that names the shortfall.

Choose one of three alphabets:

- **Letters** — adds pad letters to message letters with `C = (P + K) mod 26`. Case, spaces, punctuation, and numbers are preserved and do not consume pad.
- **Digits** — the same idea over `0..9` with `C = (P + K) mod 10`. Non-digits pass through unchanged.
- **XOR bytes** — XORs the UTF-8 bytes of the message with random pad bytes, with pad and ciphertext encoded as hex or Base64.

Leave **Pad** empty while encrypting to generate a fresh pad sized to the message. Use **Generate pad** when you only need pad material. Random pads come from the platform cryptographic RNG; generated letter and digit pads use rejection sampling so the alphabet is uniform.

### Worked example

For the classic letter example:

- Message: `HELLO`
- Pad: `XMCKA`
- Mode: `encrypt`
- Cipher: `letters`

The result is `EQNVO` because each letter is added modulo 26: `H + X = E`, `E + M = Q`, `L + C = N`, `L + K = V`, `O + A = O`. Switch to **Decrypt** with message `EQNVO` and the same pad `XMCKA` to recover `HELLO`.

### Limits and edge cases

- Maximum message/pad work is **16,384** letters, digits, or bytes per run.
- **Group output every N characters** is for display only. `0` keeps the original layout; `5` creates classic five-character groups; `20` is the maximum.
- Letter and digit modes preserve characters outside their alphabet. Those characters are not encrypted and do not consume pad.
- XOR decrypt expects the ciphertext in the selected encoding and returns UTF-8 text. If the pad or encoding is wrong, decrypted bytes may not be valid UTF-8.
- Generated pads are returned so you can store or share them out of band. Never reuse the same pad material for another message.

## FAQ

<details>
<summary>How is this different from the XOR cipher tool?</summary>

The XOR cipher tool applies a repeating key, which is useful for demos and byte manipulation but is not a one-time pad. This tool requires pad material at least as long as the transformed message and never repeats it. That strict length rule is the security property.

</details>

<details>
<summary>Why do spaces and punctuation not use up pad letters?</summary>

In the letter and digit modes only the selected alphabet is transformed. Spaces, punctuation, and other characters are copied through so formatted messages stay readable. If you need every byte encrypted, choose **XOR bytes** instead.

</details>

<details>
<summary>Can I leave the pad blank?</summary>

Yes, but only for encryption. An empty pad in **Encrypt** mode generates a fresh random pad sized to the message and returns it with the ciphertext. Decryption always requires the exact pad that was used to encrypt.

</details>

<details>
<summary>Is the generated pad cryptographically random?</summary>

The block uses the platform cryptographic random source: WebCrypto in the browser page and WASI randomness in the sandboxed tool runtime. Letter and digit pads use rejection sampling rather than a biased modulo shortcut.

</details>

<details>
<summary>What happens if the pad is longer than the message?</summary>

Only the required prefix is used and the result includes a warning. Do not treat the unused suffix as safe spare pad material unless you manage pad pages carefully outside this stateless tool; accidental reuse breaks one-time-pad security.

</details>
