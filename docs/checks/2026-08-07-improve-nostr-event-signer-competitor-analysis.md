# nostr-event-signer — competitor analysis (2026-08-07)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of publicly documented behaviour — no competitor copy,
branding, or trademarks are reproduced or reused anywhere in this tool.

## Search

One web search: *"nostr event signer online tool sign event nsec schnorr NIP-01"*, plus a fetch of
the normative spec (`nostr-protocol/nips` NIP-01) to pin the exact serialization used for the event
id. `nostrdebug.com` was in the result set but returned an unhandled client-side error and rendered
nothing, so it was **replaced** by the reference JS library that most clients actually sign with.

## Reference spec (normative, not a competitor)

NIP-01 fixes the id preimage as a compact JSON array with no whitespace:

```
[0, <pubkey hex>, <created_at>, <kind>, <tags>, <content>]
```

`id` = lowercase hex SHA-256 of that string; `sig` = 64-byte BIP-340 Schnorr signature over the
32-byte id, on secp256k1; `pubkey` is the 32-byte **x-only** key. Kind ranges: regular `1000–9999`
(and `1`, `2`, `4–44`), replaceable `0`, `3`, `10000–19999`, ephemeral `20000–29999`, addressable
`30000–39999`; `kind` is a 0–65535 integer.

## Competitors reviewed

### 1. `nak` — the "nostr army knife" CLI (fiatjaf/nak)

The de-facto command-line signer. Its `event` subcommand is the closest analogue to this tool.

| Capability | Shape | Verdict |
|---|---|---|
| `-k/--kind` | integer, **defaults to 1** (text note) | in-model → `kind` |
| `-c/--content` | event body string | in-model → `content` |
| `-t/--tag` | repeatable `name=value`, multi-value via `;` (e.g. `e=<id>;<relay>;root`) | in-model → `tags` shorthand |
| `--ts/--created_at` | unix seconds override; also natural-language ("two weeks ago") | in-model for unix seconds; NL parsing **not built** |
| `--sec` | hex, `nsec1…`, `ncryptsec`, bunker URI, or env var | in-model for hex + `nsec1…`; ncryptsec/bunker **not built** |
| `--pow N` | mines a NIP-13 `nonce` tag to N leading zero bits | in-model → `pow` |
| default output | one compact JSON object `{id,pubkey,created_at,kind,tags,content,sig}` | in-model → default output, field order matched |
| publish to relay | opens a websocket and sends the event | **out-of-model** (no network from a pure block / browser page) |
| `--musig` multi-party | musig2 co-signing | **out-of-model** (interactive multi-round protocol) |
| `--jq` | post-filters the emitted JSON | out-of-scope (compose with a JSON tool) |

### 2. NostrTool (nostrtool.com) — browser signer

A browser-only playground. Observed surface: a text-note content field, an alternative **raw event
JSON** input, private-key entry accepting `nsec`, hex, and BIP-39 mnemonics, a NIP-26 delegation
builder, a list of preset relay endpoints, and an output panel showing the note id, the signature,
and the raw signed event JSON. It carries a prominent warning against pasting a real key into a
hosted tool.

| Capability | Verdict |
|---|---|
| raw/partial event JSON as input | in-model → `template` param (its fields override the individual inputs) |
| shows note id + signature separately from the raw JSON | in-model → `report` output mode |
| private key as `nsec` or hex | in-model → `nsec` param accepts both |
| BIP-39 mnemonic → key (NIP-06 derivation) | **not built** — separate concern; belongs in a key-derivation tool |
| NIP-26 delegation tag | **not built** — needs a *second* (delegator) key and a separate token signature; NIP-26 is deprecated in practice |
| preset relay list / broadcast | **out-of-model** (no network) |
| "don't paste a real key" warning | adopted as our own independently-written page + FAQ copy |

### 3. `nostr-tools` (nbd-wtf) — the reference JS library

What most web clients sign with. `finalizeEvent(template, sk)` takes `{kind, created_at, tags,
content}` and fills in `pubkey`, `id`, `sig`; `getEventHash`, `getSignature`, `verifyEvent` and
`serializeEvent` expose the individual steps; `nip19` decodes `nsec`/encodes `npub`.

| Capability | Verdict |
|---|---|
| template = exactly `{kind, created_at, tags, content}` | in-model → our four core params use those names |
| `verifyEvent` round-trip | in-model → we **always** verify our own signature before emitting, and surface it in `report` |
| `nip19` npub/note encodings of the result | in-model → `report` prints `npub…` and `note…` alongside the hex |
| `pool.publish` to relays | **out-of-model** (no network) |

## Table-stakes → descriptor mapping

Every table-stake above lands in the descriptor or in the explicit not-built list — nothing dropped
silently.

| Param | Type | Default | Source |
|---|---|---|---|
| `nsec` | string (required) | — | nak `--sec`, NostrTool key field |
| `content` | string | `""` | all three |
| `kind` | integer 0–65535 | `1` | nak `-k` default 1 |
| `tags` | string | `""` | nak `-t` shorthand **and** the JSON array-of-arrays form |
| `created_at` | integer | `0` = sign at the current time | nak `--ts` |
| `template` | string | `""` | NostrTool raw-JSON input |
| `pow` | integer 0–20 | `0` | nak `--pow` (capped so a browser tab stays responsive) |
| `output` | enum `event`/`relay-message`/`report` | `event` | nak default JSON; the relay envelope is what a raw relay client wants; `report` mirrors NostrTool's id/sig panel |
| `pretty` | boolean | `true` | nak prints compact; a page reader wants indented — checkbox covers both |

## UX decisions taken from the scan

- **Preset chips** (`[[example]]`): kind 1 text note, kind 0 profile metadata, a reply with an `e`
  tag, and a proof-of-work note — competitors expose kinds through presets/flags rather than a
  lookup table, and chips are this repo's declarative preset answer.
- **`[input.labels]`** on `output` so the `<select>` reads as prose rather than raw enum values.
- **`multiline`** on `nsec`, `content`, `tags`, and `template` so pasted multi-line JSON/tags survive.
- **Friendly kind hints** in the page copy (0/1/3/4/7/30023) instead of an enum, since `kind` is a
  full 0–65535 integer and locking it to a list would be strictly worse than nak.

## Explicitly NOT built (out-of-model or out-of-scope)

Relay publishing over websockets; `ncryptsec` (NIP-49) and bunker/NIP-46 remote signing; musig2
co-signing; BIP-39/NIP-06 mnemonic key derivation; NIP-26 delegation tokens; natural-language
timestamp parsing; jq post-processing. The first four need network or a second interactive party;
the rest are separate tools.
