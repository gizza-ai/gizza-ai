# audio-format-conformance-checker — competitor analysis (2026-08-18)

Scan run BEFORE implementing, per `/create-next-tool` step 4. Search: "check audio file real
format codec container vs extension online tool mislabeled renamed mp3 wav" (WebSearch), then
three reachable competitor tools fetched and skimmed. **Everything below is paraphrased** — no
competitor copy, branding, or trademark is reproduced anywhere in this repo.

## Competitors skimmed

### 1. Audio file format checker (mainconverter.com/check-audio-file-format)
| aspect | observed (paraphrased) |
| --- | --- |
| features | Drop an audio file in the browser, get its real format identified from the leading signature bytes; explicitly markets catching a file whose extension disagrees with its actual bytes |
| reported fields | filename, file size, file type + MIME classification, the format signature (magic bytes), duration, channel count, sample rate, a format category |
| params/options | none beyond the file itself (upload dialog + drag-and-drop) |
| formats named | MP3, WAV, M4A, FLAC, AAC, OGG, AMR ("all common formats") |
| limits stated | none on size; states processing stays in the browser |
| help topics | what an audio format is; runs in-browser with no install; privacy; which formats are covered; how extension-vs-real-format mismatches are found |

### 2. Codec finder (free-codecs.com/app/codec-finder)
| aspect | observed (paraphrased) |
| --- | --- |
| features | Drop any media file, list every video track, audio track and subtitle stream with codec names; reads only the first few kilobytes of the header; local-only |
| reported fields | per-track codec, video profile, playback-support hint, resolution, bit rate, HDR flavour |
| params/options | drag-and-drop or file picker; an "analyse another file" action |
| formats named | MKV, MP4, AVI, MOV, WEBM, M2TS, VOB, MP3, FLAC, WAV, AAC, OGG containers; H.264, HEVC, AV1, VP9 video; Dolby (incl. TrueHD), DTS/DTS-HD, FLAC, AAC audio |
| limits stated | header-only read; no size cap given |
| mismatch check | not offered — it identifies codecs but never compares them against the filename/claim |

### 3. Audio file extension checker (advalify.io/audio-validator/file-extension)
| aspect | observed (paraphrased) |
| --- | --- |
| features | Validates that an audio ad's extension is one the delivery platforms accept; part of a larger validation suite; API access offered |
| reported fields | extension validity, a compatibility assessment, a rejection-risk indicator |
| params/options | upload / drag-and-drop only |
| formats named | MP3, WAV, OGG |
| limits stated | 100 MB per upload |
| verdict semantics | pass/fail on "extension correct vs unsupported", framed as "will this fail to play" |

A fourth result (webtoolnexus codec-detector) was skimmed from the search summary only: it guesses
codecs from the container/extension and then asks the browser
(`MediaSource.isTypeSupported` / `canPlayType`) whether it can play them.

## Table stakes → where each one landed

| table stake | source | decision |
| --- | --- | --- |
| Identify the real format from magic bytes, not the extension | 1 | **in-model** — hand-rolled signature table in `core` (audio containers in detail + a non-audio set) |
| Show the signature bytes themselves | 1 | **in-model** — `signature_hex` (first 12 bytes) |
| Report MIME type + format category | 1 | **in-model** — `detected_mime`, `detected_category` |
| Duration, sample rate, channels | 1 | **in-model** — symphonia demux via the existing `media-info` core (reused, not duplicated) |
| Per-track codec name (not just container) | 2 | **in-model** — `codec`, `track_count`; this is the capability plain magic-byte sniffing (`detect-file-type`) cannot give |
| Bit rate | 2 | **in-model** — `bitrate_kbps` |
| Explicit extension-vs-content mismatch verdict | 1, 3 | **in-model** — `verdict` + `conformant` + `suggested_extension`; the whole point of the tool |
| Pass/fail framing a human can act on | 3 | **in-model** — `summary` one-liner + `issues[]` |
| Check against a format the user claims (not just the filename) | 3 | **in-model** — `claimed_format` enum with `auto` = fall back to the filename |
| Check the codec inside the container, not only the container | 2, 3 | **in-model** — `expected_codec` enum (e.g. is this `.m4a` really AAC, or ALAC?) |
| Strict vs lenient container families | ours (gap none competitor closes) | **in-model** — `strict` boolean: `.ogg` holding Opus, or `.mp4` holding an M4A brand, pass by default and fail under `strict` |
| Drag-and-drop upload UI | 1, 2, 3 | **out-of-model here** — the generator's page file-input path is ffmpeg-only, so pure-Rust file→report tools ship chat + CLI with no page (same as `media-info`, `detect-file-type`, `image-format-validator`). Files arrive as an attachment `ref` in chat or a public `url` on the CLI |
| Browser playback-support probe (`canPlayType`) | webtoolnexus | **out-of-model** — needs live browser media APIs; nothing a wasm block can answer honestly |
| Video tracks, subtitles, HDR, Dolby TrueHD / DTS | 2 | **out-of-model** — symphonia has no DTS/TrueHD demux, and video/subtitle inventory is `media-info`'s job, not an audio conformance check |
| 100 MB uploads | 3 | **out-of-model at that size** — the wasm sandbox is 64 MiB, so the cap is 20 MiB and is stated in the parameter/skill copy instead of being discovered via an error |
| Hosted API / account | 3 | **out-of-model** — no server, no accounts; the CLI and chat surfaces are the programmatic path |

## Dup check (why this is not an existing block)

- `blocks/detect-file-type` — sniffs magic bytes for *any* file and notes an extension mismatch, but
  stops at the container: every Ogg is "Ogg media", every `ftyp M4A ` is "M4A audio". It cannot say
  which codec is inside, has no claimed-format parameter, and no expected-codec check.
- `blocks/media-info` — reports container/codec/duration/rate/channels, but makes **no claim
  comparison at all**: it is a metadata reporter, not a validator, and it errors out on
  unrecognised input instead of returning a verdict.
- `blocks/image-format-validator` — the accepted *image* analogue of exactly this tool
  (`claimed_format` + `matches_claim` + a never-throw verdict), which is the precedent this block
  follows for audio.

Decision: **build**, reusing `media-info`'s symphonia core for the stream parse (fix-at-root-cause:
one copy of the codec mapping) and adding the conformance layer on top.

## Notes

- Magic bytes are the primary signal; the stream parse is enrichment. A file whose container is
  recognised but whose codec symphonia cannot decode still gets a container verdict plus an honest
  `parse_error` — the tool never throws on bad input.
- No competitor copy, branding, logos, or trademarks were copied. Original wording throughout.
