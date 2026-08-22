## About this tool

The scrypt encrypted data format is the container the `scrypt enc` command-line utility writes, and the one `scrypt dec` and compatible libraries read. It is not bare ciphertext: every container carries a 96-byte header holding the ASCII magic `scrypt`, a version byte, the logN, r and p cost parameters, a 32-byte random salt, a truncated SHA-256 checksum over the first 48 bytes, and an HMAC-SHA256 tag over the first 64 bytes. The body is the plaintext XORed with an AES-256-CTR keystream at a zero nonce, and the file ends with a second HMAC-SHA256 over everything before it. Both the AES key and the HMAC key come from a single 64-byte scrypt output split down the middle.

That structure is why a container is self-describing. You need the passphrase to open one, but you do not need it to find out how expensive opening it will be — pick **Inspect header** and the tool reports N, r, p, the salt and the memory the file demands without deriving a key at all. This is the analogue of `scrypt info`, and it works even for files whose parameters are far too large to actually open in a browser sandbox.

Worked example: choose **Encrypt**, set data to `hello`, passphrase to `pleaseletmein`, input encoding `text`, output encoding `hex`, logN `10`, r `8`, p `1`, and paste the fixed salt `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`. The output is exactly:

```
736372797074000a0000000800000001000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1fda46ceb5d5738b6fc865e137d56ab5898f39c2cf6c77fbc8a950f80b58a7c22e3b90cd9663c66cd5fadb6557a17ca0cc5d0aae767789f0fe3f4e1eb6298b40ec5c12b45666e4c0bcb195782c41eacbc3ebcce48dad
```

Read the header straight off that string: `736372797074` is `scrypt`, `00` is the format version, `0a` is logN 10, `00000008` is r, `00000001` is p, and the next 32 bytes are the salt you supplied. Switch the operation to **Decrypt**, leave the container in the data field, keep the passphrase, and you get `hello` back.

Limits and edge cases: decoded input is capped at 4 MiB so a paste cannot exhaust browser memory. The scrypt working buffer is capped by the memory limit slider, whose hard ceiling is 64 MiB because that is the sandbox this runs in — with the standard r of 8 that means logN up to about 16, and containers written with larger parameters can be inspected here but not decrypted. Only format version 0 exists and only version 0 is accepted. A wrong passphrase is caught by the header HMAC and a modified body by the trailing file HMAC, so neither ever yields plaintext. An empty plaintext is legal and produces a 128-byte container. Leaving the salt field empty draws 32 fresh random bytes for every run, which is what you want for anything real.

## FAQ

<details>
<summary>Is this the same format as the scrypt command-line tool?</summary>

Yes. Containers produced here are byte-compatible with `scrypt enc` and can be opened with `scrypt dec`, and containers produced by that utility (or a compatible library such as `rscrypt`) can be pasted in and decrypted here. The one difference is transport: the CLI reads and writes raw files, while this page reads and writes base64 or hex, so you decode the blob to bytes on the way out.

</details>

<details>
<summary>Why do I have to pick logN, r and p instead of a time limit?</summary>

The reference utility auto-tunes its parameters by benchmarking the machine against a `-t maxtime` and `-M maxmem` budget. That measurement is wall-clock dependent, so it would make this tool non-deterministic and give different output on a phone than on a laptop. The cost parameters are exposed directly instead, and the memory limit field plays the role of `-M`: parameters that would exceed it are refused with the exact amount they need, rather than crashing the sandbox.

</details>

<details>
<summary>What does Inspect header show, and why does it not need a passphrase?</summary>

The cost parameters and salt live in the clear in the first 48 bytes of the file, protected only by a checksum, precisely so that a reader can tell how much memory decryption will need before committing to it. Inspect header parses that region, verifies the checksum, and reports logN, N, r, p, the salt, the estimated `128 * N * r` working set, and the header, ciphertext, tag and total sizes. If the file demands more memory than the current limit, it also tells you the limit to raise it to.

</details>

<details>
<summary>Can I encrypt binary data, not just text?</summary>

Yes. Set the input encoding to `hex` or `base64` and the data is decoded to raw bytes before it is sealed, so any byte string round-trips exactly. On decrypt, plaintext that is valid UTF-8 comes back as readable text; anything else is printed in the chosen output encoding. The `text` setting also auto-detects whether a pasted container is hex or base64, so you rarely have to change it when decrypting.

</details>

<details>
<summary>Should I ever fill in the salt field?</summary>

Only for tests. An empty salt field means 32 fresh random bytes per run, so encrypting the same message twice gives two different containers — that is the correct behaviour and the reason a passphrase can be reused safely across files. Supply a fixed 64-hex-character salt only when you need byte-for-byte reproducible output, such as checking this tool against a known vector.

</details>

<details>
<summary>What happens if I lose the passphrase?</summary>

The data is gone. The salt, logN, r and p are all recorded in the header, but the keys are derived from the passphrase and nothing else, and scrypt is deliberately expensive to brute-force. There is no recovery path, no escrow, and no reset — nothing about the container is stored anywhere outside the text you hold.

</details>
