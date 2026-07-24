# format-validator — competitor analysis (2026-07-23)

Tool: `format-validator` — validate whether a string is a well-formed email, URL,
IPv4/IPv6, phone number, or credit-card number (with Luhn check); auto-detect the
format when it isn't told which one to expect.

## Competitors scanned

1. **validator.js** (`validatorjs/validator.js`, npm) — the canonical JS validation
   library. Ships `isEmail`, `isURL`, `isIP(4|6)`, `isMobilePhone(locale)`,
   `isCreditCard`, plus dozens more. Per-format functions; no single "what is this"
   entry point. Locale-aware phone validation via `isMobilePhone`.
2. **Text Validator** (textrepeater.net/text-validator) — a single web tool that
   validates a string against many formats: email, URL, phone, dates, IPv4/IPv6, and
   credit-card. Closest analogue to a unified/auto-detect validator.
3. **VCC Generator — Credit Card Validator** (vccgenerator.org) — runs the Luhn
   algorithm, identifies card brand + length. Single-format.
4. **CodeBeautify — Credit Card Validator** (codebeautify.org/credit-card-validate) —
   Luhn + brand detection (AMEX/VISA/MasterCard/Discover). Single-format.
5. **utils.com — URL Validator** (validate-url.utils.com) — validates a URL is
   well-formed and parses its components (scheme, host, path). Single-format.

## Table-stakes → decision

| Capability | Competitor(s) | In our model? | Where it lands |
|---|---|---|---|
| Email well-formedness | validator.js, Text Validator | yes | `format=email`, `auto` |
| URL well-formedness + scheme/host parse | validator.js, utils.com, Text Validator | yes | `format=url`, `auto` (reports scheme + host) |
| IPv4 / IPv6 validation | validator.js, Text Validator | yes | `format=ipv4/ipv6/ip`, `auto` (reports family) |
| Phone number well-formedness | validator.js, Text Validator | yes | `format=phone`, `auto` (E.164-style digit-count check) |
| Credit card Luhn check | all card tools, validator.js, Text Validator | yes | `format=credit-card`, `auto` |
| Credit card brand detection | VCC, CodeBeautify | yes | reported in the card check note (Visa/Mastercard/Amex/Discover/JCB/Diners) |
| Auto-detect which format a string is | Text Validator | yes (our differentiator) | `format=auto` (default) — reports `detected` + a per-format checks table |
| Machine-readable output | (dev-oriented tools) | yes | `output=json` |
| Locale/region-specific national phone rules (libphonenumber) | validator.js `isMobilePhone(locale)` | **out-of-model** | listed below — needs the ~200-locale libphonenumber metadata; we do a format/length check only |
| Bulk / multi-line list validation | QuickPR, Text Validator | **out-of-model (this pass)** | one string per call; noted as a limitation on the page |
| BIN → issuer/country lookup | VCC Generator | **out-of-model** | needs an online BIN database; format check only |
| Live DNS/MX or reachability probing | ADM, email tools | **out-of-model** | this is a *syntax* validator, never touches the network (matches `email-validator`) |

## Design chosen

- `input` (string, required) — the value to validate.
- `format` (enum, default `auto`): `auto | email | url | ipv4 | ipv6 | ip | phone | credit-card`.
- `output` (enum, default `text`): `text | json`.
- `auto` runs every check, reports the highest-priority match as `detected`
  (ipv4 → ipv6 → email → url → credit-card → phone) and a full per-format table so a
  value that is well-formed under more than one format (e.g. an all-digit string) is
  shown honestly.
- Card check: Luhn + length 12–19 + brand detection, mirroring the existing
  `luhn-validate` block's brand table.

## Not a duplicate

Each existing gizza block validates ONE already-known format (`email-validator`,
`luhn-validate`, `cidr-calculator`, `phone-format`, `url-safety-inspect`). None takes an
arbitrary string and answers "which of these formats is this, and is it well-formed?"
The auto-detect + unified multi-format interface is the distinct capability here.

Copy/branding note: no competitor copy, wording, or trademarks were reused — only the
capability set was analysed.
