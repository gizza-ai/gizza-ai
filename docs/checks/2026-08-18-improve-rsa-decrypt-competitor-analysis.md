# rsa-decrypt competitor analysis — 2026-08-18

Backlog item: `rsa-decrypt` — decrypt RSA-OAEP ciphertext using a private key. Pure compute, local
cryptography.

## Sources skimmed

One WebSearch for "RSA OAEP decrypt online private key tool" and the top real tools were skimmed.
Observations are paraphrased only.

| Competitor | What it exposes | Table-stakes patterns | Fit decision |
| --- | --- | --- | --- |
| Browserling RSA decrypt utility | Text areas for private key and encrypted message, plus decrypt output. Focuses on PEM input and local-ish quick testing. | Private-key PEM textarea, ciphertext textarea, immediate plaintext output, error messages for wrong key/ciphertext. | In-model: `private_key`, `ciphertext`, `output_encoding=utf8`, clear errors. |
| CyberChef RSA Decrypt operation | Operation controls for key, passphrase, padding and hash/encoding choices within a larger recipe pipeline. | OAEP vs PKCS#1 style selection, passphrase-protected key support, binary-safe output options. | In-model: `padding`, `hash`, `passphrase`, `output_encoding=hex/base64`. Full recipe chaining is out-of-model for this single block. |
| Devglan / generic online RSA decrypt tools | Key box, encrypted text box, algorithm/padding variants, base64-oriented ciphertext examples. | Base64 ciphertext as default, PEM keys, compatibility with old PKCS#1 v1.5 inputs. | In-model: `ciphertext_encoding=auto/base64/hex`, `padding=oaep/pkcs1v15`. |

## Descriptor decisions

- `ciphertext` is required text and accepts one RSA block.
- `private_key` is required PEM text; accepted formats are PKCS#8, PKCS#1, and encrypted PKCS#8.
- `passphrase` is optional for encrypted PKCS#8 keys.
- `padding` is `oaep` (default) or `pkcs1v15` for legacy compatibility.
- `hash` is `sha256` default plus `sha384` and `sha512` for OAEP/MGF1 matching.
- `ciphertext_encoding` is `auto`, `base64`, or `hex` because competitor examples are usually base64 but developers often copy hex bytes.
- `output_encoding` is `utf8`, `hex`, or `base64` so binary plaintext is not forced through UTF-8.

## Deliberately not built

- Key generation, RSA encryption, signing, and verification are separate neighboring tools.
- Multi-step recipe chaining and hybrid file decryption are out of scope; RSA decrypts only one block.
- Browser password masking for private-key/passphrase controls is a page-platform feature, not a block parameter.

## Verification notes

The full repo generator command can exceed the 600s foreground tool timeout on this branch because it renders hundreds of existing pages. The new page was also rendered through the same generator binary against a minimal temp root containing only `blocks/rsa-decrypt`, which exercises this tool's meta/content/schema/render path without committing generated `pkg/` artifacts.
