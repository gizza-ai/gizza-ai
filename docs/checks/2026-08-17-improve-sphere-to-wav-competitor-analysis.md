# sphere-to-wav — competitor analysis (2026-08-17)

Scan performed BEFORE implementation, per the create-next-tool recipe. One web search
("convert NIST SPHERE .sph file to WAV converter sph2pipe online tool"), then the top real
tools were skimmed. Everything below is **paraphrased**; no competitor copy, branding, or
trademarks were reused.

## Competitors reviewed

| # | Tool | Kind | Reachable |
|---|------|------|-----------|
| 1 | sph2pipe v2.5 (Linguistic Data Consortium; the reference converter, vendored by Kaldi) | CLI | yes (source + readme) |
| 2 | sphfile (Python/NumPy SPH reader) | library | yes (README; PyPI project page failed to render) |
| 3 | FFmpeg `nistsphere` demuxer (`ffmpeg -i in.sph out.wav`) | CLI/library | yes (source) |
| — | convertfiles.com "NIST to WAV" (upload-based web converter) | web | **no** — HTTP 403 to the fetcher; replaced by #3 |
| — | ICSI Speech FAQ "wavfile formats" page | reference | **no** — connection refused |

## What they offer (table stakes)

| Capability | sph2pipe | sphfile | ffmpeg | In model? | Decision |
|---|---|---|---|---|---|
| Parse the 1024-byte ASCII `NIST_1A` header (`-i`/`-r`/`-sN` typed fields, `end_head`) | yes | yes | yes (subset) | in-model | **built** — full ordered field table, any field name |
| Honour `sample_byte_format` (`01` = little-endian, `10` = big-endian) | yes | partial | yes | in-model | **built** — byte-swapped to WAV's little-endian |
| `sample_coding: pcm` | yes | yes | yes | in-model | **built** (1/2/3/4-byte) |
| `sample_coding: ulaw` / `mu-law` | yes | yes | yes | in-model | **built** — G.711 decode |
| `sample_coding: alaw` | yes | no | yes | in-model | **built** — G.711 decode |
| `sample_coding: pcm,embedded-shorten-vX` | yes (bundled shorten decoder) | no ("run sph2pipe first") | demuxes to a shorten stream | **out-of-model for now** | not built — detected and reported with a named, actionable error |
| Force output encoding (`-p` 16-bit PCM / `-u` mu-law / `-a` A-law) | yes | no | yes (`-c:a`) | in-model | **built** — `encoding = pcm16 \| source \| ulaw \| alaw` |
| Single-channel extraction (`-c 1|2`) | yes | no | yes (filters) | in-model | **built** — `channel = all \| 1 \| 2 \| mono` (mono adds an averaged downmix competitors need a filter for) |
| Sample range (`-s b:e`) | yes | yes (seconds) | yes | in-model | **built** — `start_sample` / `max_samples` |
| Time range in seconds (`-t b:e`) | yes | yes | yes | in-model, **considered → rejected** | same capability as `start_sample`/`max_samples` (seconds × `sample_rate`); two extra params would only duplicate the schema. The page copy shows the multiplication. |
| Output container: WAV | yes | yes | yes | in-model | **built** (default) |
| Output container: raw/headerless | yes | no | yes | in-model | **built** — `container = raw` |
| Output containers AU / AIFF / SPHERE | yes | no | yes | in-model but **rejected** | niche for a browser converter; `raw` + WAV cover the actual re-import paths, and `aiff-to-flac` / the audio family already own AIFF. Listed here rather than silently dropped. |
| Header/format report (`sphfile.format`, `ffprobe`) | partial | yes | yes | in-model | **built** — `output = info` renders the raw field table plus derived properties and re-import commands |
| Detached header file (`sph2pipe -h hdr`) for headerless data | yes | no | no | **out-of-model** | needs a second file input; a pure page block takes one payload |
| Sample-rate conversion | no (explicitly not supported) | no | yes | out of scope | not built — this tool re-containers, it does not resample |
| Batch / multi-file conversion | yes (one file per run) | n/a | yes | **out-of-model** | the page/CLI handle one payload per call; no server, no queue |
| Upload-based web conversion (convertfiles.com model) | n/a | n/a | n/a | **out-of-model as an upload** | this block runs locally in WebAssembly; the page takes the bytes pasted as base64/hex/`data:` URI instead of uploading them anywhere |

## Defaults chosen (and why)

- `encoding = pcm16` — the reference CLI preserves the source encoding, but the point of this
  tool is "give me a WAV every player opens", and mu-law/A-law WAVs still trip some editors.
  `encoding = source` restores the sph2pipe behaviour (companding preserved, byte order fixed).
- `channel = all`, `container = wav`, `output = data_url`, `input_format = auto`,
  `byte_order = auto` (header wins; `little`/`big` override a corpus with a mislabelled header —
  a real failure mode none of the three competitors expose a switch for).
- `start_sample = 0`, `max_samples = 0` (0 = to the end).

## UX patterns worth copying (pattern only, not copy)

- CLI-style tools document the header fields they rely on; the `info` output does the same on
  the page so a user can diagnose a corpus before converting.
- Reference converters fail loudly on shorten-compressed payloads; ours names the exact
  `sample_coding` value it found and points at the decompression step, instead of emitting silence.
- Online converters lead with one-click presets; the page ships `[[example]]` chips
  (default WAV, header report, mu-law corpus side 1, raw PCM) that prefill and run.

## Worked example used on the page and in the tests

A 12-frame 8 kHz mono 16-bit big-endian (`sample_byte_format 10`) SPHERE payload converts to a
44-byte-header WAV whose samples are byte-swapped; the same input with `output = info` reports
`sample_rate 8000`, `channel_count 1`, `sample_n_bytes 2`, `sample_coding pcm`, the byte order,
and the 0.0015 s duration. Both are asserted exactly in the unit tests, the CLI check, and the
Playwright spec.

## Limits stated on the page

Decoded input is capped at 6 MiB and produced audio at 12 MiB (hex rendering at 4 MiB), because
the block runs inside a 64 MiB WebAssembly sandbox. Shorten-compressed payloads, detached
headers, and sample-rate conversion are out of scope and say so.
