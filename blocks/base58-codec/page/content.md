## What this tool does

Encode any text or bytes to **Base58** (the Bitcoin/IPFS alphabet and its
common variants), or decode a Base58 string back to the original data —
instantly, right in your browser. Nothing is sent to a server: it runs locally,
works offline, and needs no sign-up. Pick a **Mode**, an **Alphabet**, and a
**Data format**.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns your input into a Base58 string — e.g. `Hello World!` becomes `2NEpo7TZRRrLZSi2U`. |
| **decode** | Reverses it — turns a Base58 string back into the original text or bytes. |

## Alphabets

Base58 uses 58 characters and deliberately **omits the ambiguous `0` (zero),
`O` (capital o), `I` (capital i), and `l` (lowercase L)** so strings are easy to
read and copy. The variants differ only in the order of those 58 characters.

| Alphabet | Used by | Notes |
| --- | --- | --- |
| **bitcoin** (default) | Bitcoin addresses, IPFS CIDv0, Monero | The original Satoshi alphabet — what most tools mean by "Base58". |
| **ripple** | XRP Ledger | A permuted order specific to Ripple/XRP. |
| **flickr** | Flickr short URLs | Lowercase letters come before uppercase. |

Base58 has **no padding** and **preserves leading-zero bytes**: each leading
`0x00` byte becomes a leading `1` in the output (this is how Bitcoin addresses
keep their leading `1`s).

## Data format — text or raw bytes

| Format | When encoding | When decoding |
| --- | --- | --- |
| **text** (default) | Reads the input as UTF-8 text | Renders the bytes as UTF-8 text (errors if they aren't valid UTF-8) |
| **hex** | Reads the input as a hex byte string (`48 65 6c` or `0x48656c`) | Renders the decoded bytes as hex — use this for binary data |

Switch to **hex** whenever your data is binary and not readable text — for
example a raw public-key hash or a transaction ID.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `Hello World!` | encode · bitcoin | `2NEpo7TZRRrLZSi2U` |
| `2NEpo7TZRRrLZSi2U` | decode · bitcoin | `Hello World!` |
| `0x0000287fb4cd` | encode · bitcoin · hex | preserves two leading `1`s |
| `516b6fcd0f` | encode · bitcoin · hex | `ABnLTmg` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**What's the difference from Base64?** Base58 drops the easily-confused
characters (`0 O I l`) and the `+ /` symbols, so the strings are safe to copy by
hand, double-click-select, and embed in URLs. It's slightly less compact than
Base64 in exchange.

**Which alphabet do I want?** Use **bitcoin** unless you have a reason not to —
it's the alphabet used by Bitcoin, IPFS, and most tooling. Pick **ripple** for
XRP Ledger data and **flickr** for Flickr short URLs.

**My decoded output looks garbled.** The bytes probably aren't UTF-8 text.
Switch the **Data format** to **hex** to see the raw bytes.

**Does it do Base58Check?** Not yet — this tool is plain Base58 (no version byte
or checksum). For a Bitcoin address you'd add the Base58Check checksum on top.

**Why is there no padding option?** Base58 is defined without padding. Leading
zero bytes are preserved as leading `1`s instead.
