## Generate a fresh TOTP setup secret locally

Create a new **base32 TOTP secret** and the matching **otpauth:// enrollment URI** for Google Authenticator, Aegis, 1Password, Authy, and other authenticator apps. Enter the issuer/service name and the account label, then generate a copy-ready secret and URI without sending the secret anywhere.

The default 20-byte secret gives 160 bits of entropy, matching the RFC 4226 recommendation for SHA1-based HOTP/TOTP secrets. You can raise the byte length for deployments that want more entropy, and set the digits, period, or algorithm to match the service you are configuring.

### Worked examples

- `issuer=Example`, `account=alice@example.com` generates a 160-bit base32 secret and a URI like `otpauth://totp/Example:alice%40example.com?...`.
- `secret_bytes=32`, `algorithm=sha256`, `digits=8` creates a 256-bit secret for services that explicitly support SHA-256 and 8-digit codes.
- Leave issuer blank only for internal/testing setups; real authenticator entries are much easier to identify when issuer is present.

### Limits and edge cases

- `account` is required and must be valid for the shared otpauth URI builder.
- `secret_bytes` must be 10 to 64 bytes.
- `digits` must be 6, 7, or 8.
- `period` must be at least 1 second; 30 seconds is the common authenticator default.
- `algorithm` is `sha1`, `sha256`, or `sha512`; SHA1 remains the most widely compatible default.

### FAQ

<details>
<summary>Is the generated secret sent to a server?</summary>

No. The browser version uses local WebAssembly and the platform cryptographic random source. The generated base32 secret and URI are displayed to you so you can copy or import them, but the tool does not upload them.

</details>

<details>
<summary>Should I change the algorithm from SHA1?</summary>

Only if the service you are configuring says to. Many authenticator apps support SHA256 and SHA512, but SHA1 with a 160-bit secret is still the compatibility default for TOTP enrollment.

</details>

<details>
<summary>Can I scan this directly as a QR code?</summary>

This tool returns the secret and otpauth URI. To create a scannable enrollment image, paste the URI into `otpauth-qr-generator`, which renders the QR code without retyping the secret.

</details>

<details>
<summary>Why is the issuer field recommended?</summary>

Authenticator apps use the issuer and account label to name the entry. Without an issuer, multiple accounts can look similar and become hard to distinguish later.

</details>
