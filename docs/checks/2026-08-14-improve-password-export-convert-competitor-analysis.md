# password-export-convert — competitor analysis (2026-08-14)

Scan run **before** implementing, per `/create-next-tool` step 4. All findings are paraphrased
from public documentation and repository READMEs; **no competitor copy, branding or trademark is
reproduced anywhere in this repo**. Format specifications below are file-format facts (column
headers, JSON keys) published by the vendors themselves — those are interoperability requirements,
not creative copy.

**Security note for this build:** no real password export was ever read. Every fixture, example
and test in this tool uses invented sample credentials (`demo-user@example.com`,
`sample-passphrase-1`, the public RFC-4226 style base32 test string) that correspond to no real
account.

## Competitors reviewed

| # | Competitor | Shape | Reachable |
|---|---|---|---|
| 1 | Tembrica vault-format converter | Browser-side web tool | yes |
| 2 | `bw2kpxc` (iamgreggarcia) | Python CLI | yes |
| 3 | `bitwarden-to-keepassxc-converter` (thelazyoxymoron) | Python CLI | yes |
| 4 | KeePassXC native "Import → Bitwarden JSON" | Desktop app feature | yes (docs) |
| 5 | Bitwarden's own documented CSV/JSON import contract | Vendor spec | yes |

## What they do

**1. Browser-side converter (Tembrica).** The closest analogue to what we are building: converts
between six vault formats (Bitwarden JSON, KeePass CSV, Chrome CSV, Firefox CSV, 1Password CSV,
LastPass CSV) plus a generic CSV fallback, all in the page. Notable features: **source-format
auto-detection with a manual override**, a target-format dropdown, **one-click migration presets**
for the common pairs (LastPass→Bitwarden, Chrome→KeePass, 1Password→Bitwarden, Bitwarden→KeePass),
drag-and-drop *or* paste input, and download-or-copy output. It states that name, username,
password, URL and notes survive, while folder structure and 2FA secrets need separate handling; it
warns that commas/quotes inside passwords can produce empty entries because managers disagree on
CSV escaping. It collects anonymous telemetry.

**2/3. The two Python CLIs.** Single-direction (Bitwarden JSON → KeePassXC CSV). `bw2kpxc`
explicitly **does not carry TOTP** across, and flattens Bitwarden custom fields into the notes as
plain text (mapping them into KeePassXC "additional attributes" is an open TODO).
`bitwarden-to-keepassxc-converter` handles logins, secure notes and cards, putting card details
into the notes field under a separate `Cards` group; it exists mainly because the native import
mishandles organization exports. Neither offers options beyond "run it on a file".

**4. KeePassXC native import.** Since 2.7.7 it reads Bitwarden JSON directly, so a
Bitwarden→KeePass migration often needs no converter at all — but it is a desktop install, it is
one-directional, and it reportedly stumbles on organization exports.

**5. Vendor format contracts (the interoperability table-stakes).**
- Bitwarden CSV: `folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp`
  (`type` is `login` or `note`; `favorite`/`reprompt` are `1`/`0`).
- Bitwarden JSON: `{ encrypted, folders:[{id,name}], items:[{id, organizationId, folderId, type,
  reprompt, name, notes, favorite, fields, login:{uris:[{match,uri}], username, password, totp},
  collectionIds}] }`, with item `type` 1=login, 2=secure note, 3=card, 4=identity.
- KeePassXC CSV (2.6.2+): `Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created`,
  all fields quoted.
- LastPass generic CSV: `url,username,password,totp,extra,name,grouping,fav`.
- Chrome password CSV: `name,url,username,password,note`.

## Gap table — every table-stake lands in the descriptor or in the out-of-model list

| Table-stake seen | Verdict | Where it lands |
|---|---|---|
| Bitwarden JSON ⇄ KeePass CSV ⇄ generic CSV (the backlog ask) | in-model | `from` / `to` enums, both directions |
| Extra formats the browser competitor ships (Chrome, LastPass, Bitwarden CSV) | in-model | added to `from`/`to`: `chrome-csv`, `lastpass-csv`, `bitwarden-csv` |
| Source-format **auto-detection** with manual override | in-model | `from = auto` (default) sniffs JSON vs. each CSV header shape; any explicit value overrides |
| One-click **migration presets** | in-model | four `[[example]]` preset chips on the page (Bitwarden→KeePass, KeePass→Bitwarden, Chrome→Bitwarden, LastPass→KeePass) |
| Paste **or** file input | partly in-model | paste is native (`multiline` textarea); the page's download button covers the output side. A file-picker for a *text* param is a generator-wide feature, not a per-tool one — listed below |
| **TOTP carried across** (the #1 thing the CLIs drop) | in-model | `include_totp` (default on); written to every target that has a TOTP column, folded into the note for Chrome CSV which has none |
| Custom fields / card / identity data preserved | in-model | `include_extra_fields` (default on) folds Bitwarden custom fields, card and identity details, and secondary URIs into the notes — the same rescue the CLIs do, but opt-out-able |
| Folder / group structure preserved | in-model | folder ⇄ group ⇄ grouping mapped in every direction; `default_folder` names the group for entries that have none |
| Correct CSV escaping (their stated failure mode) | in-model | a real RFC-4180 reader/writer both ways, with an explicit test for a password containing a comma, a quote and a newline |
| Entries with no password breaking a target import | in-model | `skip_empty_passwords` |
| Favorites | in-model | carried through `fav`/`favorite`/`Icon`-free targets where a column exists |
| Deterministic, re-runnable output | in-model | folder/item ids are hashed from names, not random, so the same export always converts to the same bytes |
| 1Password `.1pux` / Firefox CSV | considered, rejected | `.1pux` is a zip archive, not a paste-able text export; Firefox CSV is Chrome CSV plus dead metadata columns, which the generic CSV alias reader already accepts on input |
| Encrypted Bitwarden exports (password-protected JSON) | out-of-model here | decryption belongs to a crypto tool, not a format converter; the page says so and points at exporting unencrypted |
| Drag-and-drop file upload of a vault export | out-of-model here | the page's text-param surface has no file picker; adding one is a shared-generator change, deliberately out of scope for a tool build |
| Telemetry | rejected on principle | nothing is measured, sent or stored; conversion is wasm in the tab |
| Direct write into a `.kdbx` database | out-of-model | needs the KeePass database format + a master key, i.e. a different tool |

## Decisions

- Six formats in, six formats out, any direction, with auto-detection — matching the broadest
  competitor rather than the single-direction CLIs.
- TOTP and custom-field survival are the differentiators: both leading CLIs lose one or the other.
- Output is the converted file **and nothing else** — no header comment, no summary line — so it
  can be saved and imported verbatim.
- Copy, examples and FAQ are original, and every sample credential is invented.
