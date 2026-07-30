## About this tool

Certificate Chain Validator checks the local structure of PEM encoded X.509 chains. Paste the leaf certificate first, followed by each issuing intermediate and the optional self-signed root. The tool confirms that every certificate is currently inside its validity window, each child issuer matches the next certificate subject, each issuer certificate has `basicConstraints CA:true`, and each signature verifies with the next certificate's public key.

This is useful when you are debugging TLS deployments, mTLS client certificates, private PKI bundles, or ACME automation output and want an offline first-pass check before involving a browser, load balancer, or operating-system trust store.

Worked example:

1. Export a chain as PEM, ordered leaf to root.
2. Paste every `-----BEGIN CERTIFICATE-----` block into the input.
3. Run the validator.
4. A successful report starts with `Certificate chain: VALID` and lists each certificate's subject, issuer, serial number, validity window and CA status.

Limits and edge cases:

- This tool does not contact AIA URLs, OCSP responders, CRLs, CT logs, DNS, or HTTPS endpoints.
- It does not decide whether the root is trusted by your browser, operating system, runtime, or application.
- It validates common signature algorithms supported by the underlying parser; unsupported or malformed signatures are reported as validation errors.
- The expected order is leaf, then intermediates, then optional root. Reversed bundles fail with an issuer/subject mismatch.

## FAQ

<details>
<summary>Does this prove my certificate is trusted by browsers?</summary>

No. It verifies the pasted chain's internal ordering, signatures and dates. Browser trust also depends on root-store membership, name matching, key usages, revocation policy, certificate transparency and platform-specific rules.

</details>

<details>
<summary>What order should I paste the certificates in?</summary>

Paste the leaf certificate first, then each intermediate issuer, and finally the self-signed root if you have it. Many servers store bundles in that order; root-to-leaf bundles should be reversed before checking.

</details>

<details>
<summary>Does the tool send certificates to a server?</summary>

No. The page runs the parser and validation code in WebAssembly in your browser. The CLI path runs the same Rust logic locally.

</details>

<details>
<summary>Why can a valid chain still fail in production?</summary>

Production clients can reject chains for reasons outside this offline model: hostname mismatch, missing EKU, distrusted roots, policy constraints, revocation, clock skew, or a server that sends an incomplete chain.

</details>
