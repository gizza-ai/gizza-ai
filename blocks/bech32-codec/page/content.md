## What this tool does

Encode a **human-readable prefix (HRP)** plus a data payload into a checksummed
**Bech32** (BIP 173) or **Bech32m** (BIP 350) string, or decode and validate an
existing Bech32 string back into its prefix, checksum variant, and data.

Bech32 is the encoding behind SegWit Bitcoin addresses (`bc1…`), Taproot
addresses, Lightning invoices (`lnbc…`), Nostr keys (`npub…` / `nsec…`), and many
Cosmos-ecosystem addresses. It packs bytes into a base-32 alphabet, adds a `1`
separator after the prefix, and appends a 6-character BCH checksum that reliably
catches typos and altered characters.

Everything runs locally in WebAssembly. Your input is never uploaded.

## Encode vs decode

- **Encode** (default): supply a prefix (HRP) such as `bc`, a data payload, and a
  variant. The tool converts the bytes to 5-bit groups (BIP 173 `convertbits`),
  computes the checksum, and returns the full Bech32 string.
- **Decode**: paste a Bech32 or Bech32m string. The tool validates its structure,
  case, alphabet, and checksum, then returns the detected HRP, the checksum
  variant it matched, and the decoded data. The `hrp` and `variant` fields are
  ignored on decode — both are read from the string itself.

## Bech32 vs Bech32m

| Variant | Checksum constant | Used by |
| --- | --- | --- |
| **Bech32** | `1` | SegWit v0 outputs — native P2WPKH / P2WSH addresses. |
| **Bech32m** | `0x2bc830a3` | SegWit v1+ (Taproot) addresses and newer schemes. |

Bech32m was introduced in BIP 350 to fix an insertion-weakness in the original
checksum. On decode the correct variant is detected automatically.

## Data formats

- **hex** (default): the payload is raw bytes written as hex (`751e76…`, spaces
  and a leading `0x` are tolerated). Best for binary data like public-key hashes.
- **text**: the payload is UTF-8 text. On decode, choose text only when the bytes
  are valid UTF-8; otherwise use hex to inspect the raw bytes.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `751e76e8199196d454941c45d1b3a323f1433bd6` | encode · bc · bech32 · hex | `bc1w508d6qejxtdg4y5r3zarvary0c5xw7kj7gz7z` |
| `bc1w508d6qejxtdg4y5r3zarvary0c5xw7kj7gz7z` | decode · hex | `hrp: bc` · `variant: bech32` · `data: 751e76…3bd6` |
| `hello` | encode · test · bech32m · text | `test1dpjkcmr0scqr9j` |
| `test1dpjkcmr0scqr9j` | decode · text | `hrp: test` · `variant: bech32m` · `data: hello` |

## Tips

- The HRP is only used when encoding — on decode it is read from the string, so
  you can leave the prefix field blank.
- Bech32 strings must be all-lowercase or all-uppercase; a mixed-case string is
  rejected by design (BIP 173) because it would break the checksum guarantee.
- The alphabet deliberately excludes `1`, `b`, `i`, and `o` to avoid visual
  confusion, so those characters never appear in the data part.

## Frequently asked questions

<details>
<summary>Is this the same as generating a Bitcoin address?</summary>
<p>Not quite. A full SegWit address prepends a witness-version symbol to the
program before Bech32-encoding it. This tool encodes the raw data you give it
under the prefix you choose, which is the underlying primitive — handy for
inspecting, learning, or building the address layer yourself.</p>
</details>

<details>
<summary>Which variant should I use — Bech32 or Bech32m?</summary>
<p>Use <strong>Bech32</strong> for SegWit v0 (legacy native SegWit) and
<strong>Bech32m</strong> for SegWit v1+ / Taproot and other newer schemes. When
decoding you don't have to choose — the correct variant is detected from the
checksum and reported back to you.</p>
</details>

<details>
<summary>Why does decoding sometimes fail with a checksum error?</summary>
<p>A single altered, inserted, or dropped character changes the BCH checksum, so
the string no longer validates. That is the whole point of Bech32: it catches
typos before you send funds to a wrong address.</p>
</details>

<details>
<summary>Can I decode a Nostr npub or a Lightning invoice?</summary>
<p>Yes — paste it and decode. You'll get the prefix (e.g. <code>npub</code> or
<code>lnbc</code>), the variant, and the raw data bytes. Nostr keys use Bech32;
switch the data format to <strong>hex</strong> to read the 32-byte key.</p>
</details>

<details>
<summary>Does my data leave my device?</summary>
<p>No. The encoder/decoder is compiled to WebAssembly and runs entirely in your
browser. Nothing is uploaded to a server.</p>
</details>
