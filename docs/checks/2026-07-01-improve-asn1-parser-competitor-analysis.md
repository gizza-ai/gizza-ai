# Competitor analysis: asn1-parser

Date: 2026-07-01
Tool: `asn1-parser`

## Competitors reviewed

| competitor | useful capabilities observed | gaps considered |
| --- | --- | --- |
| lapo.it ASN.1 JavaScript decoder | Accepts hex, raw/base64 and PEM-like inputs; displays a navigable ASN.1 tree; resolves many common OIDs; runs client-side in JavaScript. | PEM/base64 autodetection and expandable UI are useful, but this initial gizza tool keeps the deterministic chat/CLI/page input as hex and provides tree/JSON outputs locally. |
| olibu.github.io ASN.1 JavaScript decoder | A maintained mirror/fork of the classic JavaScript decoder with DER/BER tree decoding and hex/base64 input support. | Similar PEM/base64 convenience; no account/server requirements. Hex support and tree output are covered. |
| 8gwifi ASN.1 Decoder | Web form for DER/BER and X.509/CSR inspection with expandable tree and OID labels. | Upload/paste PEM certificate workflows are out of scope for the first browser-local pure-text version; OID labels and nested tree are in-model and implemented. |
| Encryption Consulting ASN.1 Decoder | Certificate/CSR-oriented workflow that accepts pasted or uploaded PEM/DER/hex data. | File upload and certificate-specific presentation could be a future page enhancement; current gizza tool focuses on raw ASN.1 TLV decoding across chat, CLI and page. |
| uFreeTools ASN.1 Structure Parser | Tool positions itself around ASN.1 visualization, PEM/HEX/Base64/DER formats and export-oriented analysis. | Multiple input encodings and richer export UI are useful future ideas; JSON output covers a scriptable export format now. |

## In-model gaps closed

- Added a local ASN.1/DER TLV parser for hex input with tolerance for spaces, colons, separators and `0x` prefixes.
- Rendered an indented tree with tag label, raw tag byte, byte length, offset and decoded primitive values.
- Added structured JSON output for scripting and inspection.
- Decoded common universal primitives: INTEGER, BOOLEAN, NULL, OBJECT IDENTIFIER, RELATIVE-OID, strings, time strings, ENUMERATED and BIT STRING summaries.
- Added friendly labels for common PKI/X.509 OIDs including commonName, RSA/ECDSA/EdDSA algorithms, certificate extensions and EKU values.
- Recurses through constructed SEQUENCE/SET/context-specific structures and best-effort encapsulated BIT/OCTET STRING DER.
- Added browser page metadata/content, wafer fixtures, CLI smoke path and Playwright page coverage including a query-parameter deep link.

## Out-of-model or deferred gaps

- PEM/base64/DER file upload convenience is deferred; the current descriptor intentionally accepts hex text so the same schema works in chat, CLI and page.
- Rich expandable/collapsible tree UI is deferred; current page output is a stable text/JSON rendering using the standard gizza page runtime.
- Certificate-specific semantic validation (issuer/subject/date/key-usage summaries) is deferred; the parser remains a generic ASN.1 TLV inspector.

## Verification notes

- Unit tests cover primitive decoding, nested sequences, OIDs, separators, JSON shape and error cases.
- Drift guard pins the chat/CLI schema: required `input` string plus optional `format` enum (`tree`/`json`, default `tree`).
- Page tests cover tree output, OID labels, JSON output and URL query-param prefill.

Original analysis only; no competitor copy, branding or assets were copied.
