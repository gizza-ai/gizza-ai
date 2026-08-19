## About this tool

Shamir Secret Recover combines threshold shares back into the original secret. Paste the shares you already have — one per line — and the recovery runs locally in your browser. The tool does not generate new shares, store anything, contact a server, resolve files, or try to interpret key formats.

It implements the common byte-wise Shamir scheme over GF(256), using Lagrange interpolation at x = 0. The parser covers the share layouts most often seen in browser and operations tools: index-prefixed hex such as `1-deadbeef`, `sss:` base64url shares whose first decoded byte is the x coordinate, and raw hex/base64 shares whose last decoded byte is the coordinate.

When you provide more shares than the threshold, the **Cross-check redundant shares** option reconstructs from alternate subsets and refuses to return a secret if a corrupted or foreign share disagrees. That closes the main practical gap in many combine tools: without redundancy, a modified share can produce a plausible but wrong secret.

### Worked example

Paste these 3-of-5 shares:

```
1-68b509858f664dea3c3829
2-73fb23909fc0cdded4b830
3-732b46797f86f75b9aec7d
```

Set **Threshold K** to `3` and leave format, encoding and polynomial on auto. The secret-only output is:

```
hello world
```

For a diagnostic view, switch **Output** to **Report**. The report shows which share layout was used, which GF(256) polynomial was chosen, the share indices, and whether redundant-share verification passed.

### Limits and edge cases

- **Recover-only.** This tool combines existing shares; it does not split a new secret or create printable share packets.
- **GF(256) byte-wise shares only.** It supports 0x11b and 0x11d reduction polynomials. `ssss`, `secrets.js` packed-header shares, SLIP-39 mnemonic shares and prime-field classroom examples use different schemes and are listed as unsupported rather than guessed.
- **At most 255 shares.** GF(256) has only 255 nonzero x coordinates. Duplicate x values are rejected.
- **All payloads must be the same length.** A Shamir share is the same byte length as the secret.
- **Threshold is explicit when you have extras.** Use `0` to combine every supplied share, or set K to the real threshold when you pasted more than K shares so cross-checking can use the spare shares.
- **Integrity needs redundancy.** If you provide exactly K shares, there is no independent way to know whether one share was modified. Use a report output and keep verification on whenever you have extra shares.

## FAQ

<details>
<summary>Can this recover shares from any Shamir implementation?</summary>

No. Shamir is a family of schemes, not one wire format. This tool supports byte-wise GF(256) shares with x coordinates stored as a prefix byte, suffix byte, or decimal prefix. It does not implement `ssss` point-at-infinity shares, SLIP-39 mnemonics, `secrets.js` packed headers, or prime-field decimal demos.

</details>

<details>
<summary>Why does the polynomial matter?</summary>

GF(256) arithmetic needs a reduction polynomial. Two common choices are `0x11b` and `0x11d`; using the wrong one returns different bytes without necessarily looking like an error. Leave the field on auto when you have redundant shares, or choose the polynomial used by the tool that created your shares.

</details>

<details>
<summary>Why does verification need more shares than the threshold?</summary>

With exactly K shares, every set of bytes defines some possible secret. A corrupted share simply reconstructs a different value. With K+1 or more shares, the tool can leave shares out and check that independent subsets recover the same secret, which detects many copy/paste errors and foreign shares.

</details>

<details>
<summary>What should I choose for secret encoding?</summary>

Use **Auto** for passwords, tokens and short notes: printable UTF-8 is shown as text and binary data falls back to hex. Choose **Hex** or **Base64** for keys and random bytes, and **Text** when you want the tool to fail loudly if the recovered value is not valid UTF-8.

</details>
