# youtube-id-extractor — competitor analysis (2026-09-23)

Scan run **before** implementation, per the create-next-tool recipe. All observations are
paraphrased from public product pages; no competitor copy, branding or trademark text is reused.

## Tools reviewed

| # | Tool | Surface |
|---|------|---------|
| 1 | TimeSkip "YouTube Video ID Finder" (timeskip.io) | single-URL box, instant result |
| 2 | Handytool "YouTube Video & Channel ID Extractor" (handytool.io) | single-URL box, multi-field result card |
| 3 | Terrific Tools "YouTube Video ID Finder" (terrific.tools) | bulk textarea, up to 100 URLs |
| — | labnol.org "RegEx — Extract Video ID from YouTube URLs" | reference regex, not a product |

A fourth candidate (freemediatools.com bulk extractor) returned HTTP 404 and was replaced by
Terrific Tools so the scan still covers three reachable tools.

## Table-stakes matrix

| Capability | Seen on | Fit | Where it landed |
|---|---|---|---|
| Extract the 11-char video ID | 1, 2, 3 | in-model | `id` field, validated `[A-Za-z0-9_-]{11}` |
| `youtube.com/watch?v=` | 1, 2, 3 | in-model | parser |
| `youtu.be/<id>` short links | 1, 2, 3 | in-model | parser |
| `/shorts/<id>` | 1, 2, 3 | in-model | parser |
| `/embed/<id>` | 1, 2 | in-model | parser |
| `/live/<id>` | 1, 2 | in-model | parser |
| `/v/<id>`, `/e/<id>` | labnol | in-model | parser |
| `youtube-nocookie.com` | labnol | in-model | parser (host-family match) |
| `m.` / `music.` / `gaming.` subdomains | 2 | in-model | parser (any subdomain of the family) |
| Bare 11-char ID pasted directly | 2 | in-model | parser (bare-token branch) |
| Channel ID (`/channel/UC…`) | 2 | in-model | `kind = channel` |
| `@handle`, `/user/`, `/c/` | 2 | in-model | `kind = handle` / `user` / `custom` |
| Playlist ID (`list=`) | 2 | in-model | `playlist` field + `kind = playlist` |
| Start timestamp (`t=`, `start=`) | 2 | in-model | `start` — `90s (1:30)` |
| Timestamp in seconds **and** clock form | 2 | in-model | `timestamp` param: `seconds`/`clock`/`both` |
| Canonical watch URL (timestamp preserved) | 2 | in-model | `canonical` toggle, on by default |
| Thumbnail URL (`hqdefault`) | 2 | in-model | `thumbnail` param — 5 variants + `none` |
| Bulk / one URL per line | 3 | in-model | `urls` is multi-line, 200-line cap |
| Browser-local, no upload, no API key | 1, 2, 3 | in-model | wasm, no network on any surface |
| "Load sample" / preset button | 2 | in-model | four `[[example]]` chips on the page |

## Gaps we close that none of the three ship

- **Invidious / Piped and any other front-end host.** Competitors hard-code YouTube hostnames. We
  apply the same path/query rules to *any* host, so `yewtu.be/watch?v=…`, `piped.video/watch?v=…`
  and `inv.nadeko.net/<id>` resolve. This is the row's stated scope and the main differentiator.
- **Machine-readable output.** `format = json | csv` alongside the human `text` report — none of
  the three export structured data (Terrific Tools is on-screen list only).
- **Wrapper URL unwrapping.** `/attribution_link?u=…`, `/redirect?q=…` and `/oembed?url=…` are
  decoded and re-parsed (depth-limited), which no scanned tool does.
- **Privacy-enhanced embed URL** (`youtube-nocookie.com/embed/…`, carrying `list` + `start`) as an
  opt-in output field.
- **Strict mode** — fail the whole run on any unparseable line, for pipeline use.
- **Unit-suffix and colon timestamps** (`1m30s`, `1h2m3s`, `1:02:03`, `#t=…`), not just bare
  seconds.

## Out-of-model (listed, not built)

- **Live lookup of title / duration / channel name / view count.** Requires the YouTube Data API
  with a key and a network round-trip; gizza blocks are deterministic local compute and the page
  runtime has no third-party fetch surface. Same class as the skiplisted `youtube-transcript`.
- **Verifying that an ID actually exists / is public.** Needs a live request to youtube.com.
- **Resolving `@handle`, `/user/` or `/c/` to the underlying `UC…` channel ID.** That mapping only
  exists server-side; we report the handle/name verbatim instead of guessing.
- **Thumbnail image preview.** The page output is a single text pane; we emit the thumbnail URL,
  not the fetched image.

## Decisions

- Classified **pure**, not `ffmpeg` — the picker's `type_hint` is a false positive, this is string
  parsing with no media involved.
- `thumbnail` defaults to `hqdefault` (the variant that exists for every video; `maxresdefault`
  404s on older uploads — stated on the page).
- `canonical` defaults **on**, `embed` defaults **off**: the canonical link is the common follow-up
  action, the nocookie embed is a narrower need.
- Not a duplicate: `url-cleaner` strips tracking params, `extract-urls`/`link-extractor` pull links
  out of text, `safelink-decoder` unwraps mail-gateway redirects, `gdrive-link-converter` and
  `cloud-share-direct-link` rewrite cloud share links, and `youtube-thumbnail-generator` composites
  an image with ffmpeg. None parses a YouTube URL into its video ID and start offset.
