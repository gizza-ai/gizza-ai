## About this tool

A **magnet link** is a `magnet:?…` URI that identifies a BitTorrent download by
its content hash rather than by a `.torrent` file on a server. This tool reads
one apart into its pieces — or assembles a fresh one from the parts you supply —
entirely in your browser. Nothing you paste is uploaded.

### Parse mode

Paste a magnet link and get back every component, decoded:

- **Info hash (v1)** — the `urn:btih:` value, normalised to lower-case hex
  whether the link used 40 hex characters or 32 base32 characters, with the
  original encoding noted.
- **Info hash (v2)** — the `urn:btmh:` multihash, for BitTorrent v2 links.
- **Display name** (`dn`) — percent- and `+`-decoded back to readable text.
- **Trackers** (`tr`) — every announce URL, in order.
- **Web seeds** (`ws`) — HTTP/FTP seed URLs (BEP 19).
- **Acceptable / exact sources** (`as` / `xs`), **keywords** (`kt`), and the
  **exact length** (`xl`) in bytes, shown with a human-readable size.
- Any other parameters are listed verbatim, and indexed keys such as `tr.1` and
  `tr.2` are collapsed to their base key.

### Build mode

Switch the mode to **build**, enter an info-hash (40 hex characters, 32 base32
characters, or a full `urn:btih:…` value) and any optional display name,
trackers, web seeds and exact length, and the tool returns a ready-to-use
`magnet:?…` link with every field correctly percent-encoded.

### Privacy

Everything runs locally in WebAssembly. No magnet link, hash, or tracker URL
ever leaves your device, and there is no sign-up.

## FAQ

<details>
<summary>Why is the reported info hash different from what's in my link?</summary>

It's the same hash, normalised. A v1 (`urn:btih:`) info-hash can be written as 40
hex characters or 32 base32 characters; the parser converts both to lower-case
hex so they're directly comparable, and notes which encoding the link originally
used. If your link used base32, the 40-hex output is the canonical form of the
identical value.

</details>

<details>
<summary>Are BitTorrent v2 and hybrid magnet links supported?</summary>

Yes. A v2 link's `urn:btmh:` multihash is reported separately as the v2 info
hash, and a hybrid link — one carrying both `urn:btih:` and `urn:btmh:` exact
topics — shows both hashes. Other URN namespaces in `xt` (e.g. `ed2k`, `sha1`)
are listed and classified rather than dropped.

</details>

<details>
<summary>Does the tool contact trackers or download anything?</summary>

No. It only reads (or assembles) the URI text — trackers are never announced to,
no peers are contacted, and no torrent metadata is fetched. That's why it can
show what a magnet link *points at*, but not the file list inside the torrent.

</details>

<details>
<summary>What formats does build mode accept for the info-hash?</summary>

Any of three: 40 hex characters, 32 base32 characters, or a complete
`urn:btih:…` value. Add an optional display name, tracker URLs, web seeds and
exact length, and the resulting `magnet:?…` link comes back with every field
correctly percent-encoded and ready to paste into a torrent client.

</details>
