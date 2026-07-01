## About this tool

**XOR cipher** applies a repeating bytewise XOR: every byte of your data is
XOR-ed with the next byte of the key, and the key repeats until the whole input
is transformed. Choose whether the data and key are read as text, hex, or
Base64, then choose hex, Base64, or UTF-8 for the result.

- **Symmetric:** XOR is its own inverse. To decrypt, run the ciphertext through
  the same operation with the same key and matching input format.
- **Flexible encodings:** use UTF-8 text for quick notes, hex for byte-oriented
  CTF exercises, or Base64 for compact copy/paste.
- **Repeating-key behavior:** a one-byte key applies the same mask to every
  byte; longer keys cycle across the data.

### Examples

- `Hello` with text key `K` outputs `032e272724` as hex. Feed that hex back in
  with key `K` and UTF-8 output to recover `Hello`.
- The CryptoPals Set 1 Challenge 5 vector is a repeating-key XOR with key `ICE`.
- For binary-looking output, prefer hex or Base64. UTF-8 output is for plaintext
  recovery and reports an error when the bytes are not valid UTF-8.

### Security warning

Repeating-key XOR is **not secure encryption**. It is useful for interop,
obfuscation, learning, and CTFs, but it is vulnerable to frequency analysis and
known-plaintext attacks. For real secrets, use authenticated encryption tools
such as **aes-cipher** or **text-encrypt** instead.

### Privacy

The browser version runs locally in WebAssembly: your data and key stay on your
device.

## FAQ

<details>
<summary>How do I decrypt something that was XOR-ed?</summary>

Run the ciphertext back through the tool with the **same key** — XOR is its own
inverse. The only detail to get right is the formats: if you produced hex
output, set the *input* format to hex on the way back, and pick UTF-8 output to
read the recovered plaintext. UTF-8 output errors if the bytes aren't valid
text, which usually means the key or an encoding setting is wrong.

</details>

<details>
<summary>Can data and key use different encodings?</summary>

Yes — the data format and key format are independent settings. You can XOR a
Base64 blob against a plain-text key, or hex data against a hex key (the classic
CTF setup). An empty key is rejected; a malformed hex or Base64 string reports a
decode error before any XOR happens.

</details>

<details>
<summary>What does a one-byte key actually do?</summary>

It XORs every byte of the input with that single value — the simplest mask, and
the one single-byte-XOR CTF challenges expect you to brute-force. Longer keys
cycle: byte *n* of the data is XOR-ed with byte *n mod keylen* of the key, which
is exactly the CryptoPals "repeating-key XOR" construction (their Set 1
Challenge 5 vector with key `ICE` reproduces here).

</details>

<details>
<summary>Is XOR encryption safe for real secrets?</summary>

No. Repeating-key XOR falls to frequency analysis and a single known-plaintext
crib — it's for learning, interop, and obfuscation only. For anything you
actually need to protect, use an authenticated cipher (see the aes-cipher or
text-encrypt tools).

</details>
