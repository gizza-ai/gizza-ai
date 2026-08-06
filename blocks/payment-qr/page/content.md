## About this tool

Payment QR codes are just payment URIs encoded as a QR image. This tool builds the URI for you, validates the destination where validation is possible, and renders a crisp SVG QR code that can be scanned from a screen or printed.

It supports Bitcoin-style BIP21 URIs (`bitcoin:address?amount=&label=&message=`), Litecoin, Dogecoin, Ethereum EIP-681, Lightning invoices, and a plain-text mode for any payload you want to encode. Everything runs locally in WebAssembly; the address or invoice never leaves your browser.

## Worked example

A Bitcoin request with:

- address `bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4`
- amount `0.025`
- label `Coffee Bar`
- message `Invoice 2026-114`

builds this payment URI before encoding it in the SVG QR code:

```text
bitcoin:bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4?amount=0.025&label=Coffee%20Bar&message=Invoice%202026-114
```

The QR itself includes that URI in the SVG `<title>` for accessibility, and the **Print URI under the code** option also draws readable wrapped text below the code.

## Options

| Option | What it does |
| --- | --- |
| **Payment scheme** | Choose Bitcoin, Litecoin, Dogecoin, Ethereum, Lightning, or plain text. |
| **Amount** | Optional coin amount. BTC/LTC/DOGE allow up to 8 decimal places; ETH allows up to 18 and is converted to exact wei. Lightning invoices carry their own amount. |
| **Label / message** | Optional BIP21 fields for Bitcoin-style schemes. They are UTF-8 percent-encoded with spaces as `%20`, never `+`. |
| **Error correction** | L is smallest, M is a balanced default, Q/H are more robust for print but make the code denser. |
| **Size and colors** | Control the SVG viewport and QR colors. Keep strong contrast; low-contrast decorative codes often fail to scan. |

## Limits and edge cases

- **Validation catches typos, not intent.** Base58Check and Bech32/Bech32m checksums catch malformed Bitcoin-style addresses, and Ethereum addresses must be 20-byte hex. The tool cannot prove the address belongs to the person you meant to pay.
- **No payment monitoring.** A QR code cannot tell you whether funds arrived. Verify payments in your wallet, node, or block explorer.
- **Amounts are requests.** Wallets usually let the payer edit the amount before sending.
- **Lightning invoices are already complete payloads.** The amount, memo and expiry live inside the BOLT11 invoice, so `amount`, `label` and `message` are rejected for `scheme=lightning`.
- **No hosted dynamic QR.** The output is a static SVG. Dynamic redirects, accounts, analytics and plan-gated downloads require a backend and are outside this local tool.

## FAQ

<details>
<summary>What is BIP21?</summary>

BIP21 is the URI format many Bitcoin wallets understand: `bitcoin:address` plus optional query parameters like `amount`, `label` and `message`. This tool uses the same grammar for Bitcoin, Litecoin and Dogecoin payment requests.

</details>

<details>
<summary>Can I use this for Ethereum?</summary>

Yes. Choose **Ethereum EIP-681** and paste a `0x` address. If you enter an amount, the tool converts decimal ETH into exact wei for the `value=` parameter without floating-point rounding.

</details>

<details>
<summary>Does the QR prove the address is safe?</summary>

No. It only proves the payload is well-formed enough to encode. A checksum catches many typing mistakes, but you still need to verify the destination with your counterparty before displaying or printing the QR.

</details>

<details>
<summary>Which error-correction level should I pick?</summary>

Use **M** for normal screen use. Use **Q** or **H** for printed codes, stickers, receipts or anything that may be smudged. Higher recovery levels make the QR denser, so keep the printed size large enough to scan.

</details>

<details>
<summary>Can I make a transparent or branded QR?</summary>

You can set the background color to `transparent` and choose foreground/background colors. Keep enough contrast for scanners. Logo overlays and styled template galleries need a second image input and are intentionally left to decorative QR tools.

</details>
