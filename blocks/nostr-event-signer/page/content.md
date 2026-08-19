## About this tool

This tool builds and signs a **Nostr event** without contacting a relay. Paste a
disposable `nsec1…` key (or a 64-character hex secret key), fill in the event
content, kind, tags and timestamp, and it returns publish-ready JSON containing
`id`, `pubkey`, `created_at`, `kind`, `tags`, `content` and `sig`.

The event id is computed exactly as NIP-01 specifies: the compact serialization
`[0, pubkey, created_at, kind, tags, content]` is hashed with SHA-256. The tool
then signs that 32-byte id with a BIP-340 Schnorr signature on secp256k1 and
verifies the signature before emitting anything.

Use a test key unless you fully trust the machine where you run this. Everything
runs locally in WebAssembly, but a private key is still a private key.

### Worked example

With this throwaway secret key:

```
nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5
```

and:

- **Content** = `hello from gizza`
- **Kind** = `1`
- **Tags** = `t=tools`
- **Timestamp** = `1700000000`

it emits a signed event JSON object. The `pubkey` is derived from the secret key,
`id` is the NIP-01 event hash, and `sig` is a 128-character Schnorr signature.
Switch **Output** to **Relay EVENT frame** to wrap the same object as:

```json
["EVENT", { "id": "…", "pubkey": "…", "sig": "…" }]
```

which is the frame shape a relay websocket expects.

### Tags

Tags can be pasted two ways:

- Shorthand, one per line or comma-separated: `t=tools` or
  `e=<event id>;wss://relay.example.com;root`.
- Full JSON array form: `[["t","tools"],["p","<pubkey hex>"]]`.

Use JSON form when a value itself contains a comma or newline.

### Raw template input

If you already have an unsigned event object, paste it into **Unsigned event
template**. Fields present in the template (`kind`, `content`, `tags`,
`created_at`) override the individual controls, while `id`, `pubkey` and `sig`
are always recomputed from the signing key.

### Limits and edge cases

- `created_at = 0` means “use the browser's current Unix time”. Pass an explicit
  timestamp for reproducible ids and signatures.
- Proof-of-work mining is capped at 20 leading zero bits so the browser tab does
  not behave like a dedicated miner.
- Relay publishing is deliberately not included. Copy the output into a relay
  client or websocket tool if you want to broadcast it.
- Encrypted secret-key formats (`ncryptsec`) and remote signing protocols are not
  implemented; this tool signs directly from an `nsec` or hex secret.

## FAQ

<details>
<summary>Can I paste my real Nostr private key here?</summary>

Technically yes, but you should prefer a disposable test key. The tool runs
locally in your browser and does not upload the key, yet any page or machine
where you paste an `nsec` has enough information to sign as you. Treat it like a
wallet seed.

</details>

<details>
<summary>What does the event id sign?</summary>

NIP-01 signs the SHA-256 hash of a compact JSON array:
`[0, pubkey, created_at, kind, tags, content]`. This tool constructs that array,
hashes it to get `id`, signs the 32-byte id with BIP-340 Schnorr, and verifies
the signature before returning the event.

</details>

<details>
<summary>How do I add reply or mention tags?</summary>

Use shorthand such as `e=<event id>;wss://relay.example.com;root` for an event
tag or `p=<pubkey hex>` for a pubkey tag. For exact control, paste full JSON tag
arrays like `[["e","<id>","wss://relay.example.com","root"],["p","<pubkey>"]]`.

</details>

<details>
<summary>What is proof of work here?</summary>

NIP-13 proof of work means changing a `nonce` tag until the event id begins with
a requested number of zero bits. This can get slow quickly, so the page caps the
request at 20 bits and replaces any existing `nonce` tag when mining.

</details>

<details>
<summary>Does this publish to relays?</summary>

No. It only builds and signs the event or relay frame. Publishing requires a
network connection to one or more relays, which is outside this local pure-WASM
block. Copy the JSON into a relay client if you want to send it.

</details>
