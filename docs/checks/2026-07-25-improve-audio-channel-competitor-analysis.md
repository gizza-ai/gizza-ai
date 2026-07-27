# audio-channel — competitor analysis (2026-07-25)

One WebSearch ("swap stereo channels / stereo to mono / left right channel online tool");
skimmed the top real tools: online channel swappers/routers, stereo↔mono converters, and
"fix one-sided audio" utilities (browser and desktop ffmpeg wrappers). No competitor copy or
branding was reproduced — only capability presence was noted.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Swap left / right channels | channel swappers | in-model | `operation=swap` → `pan=stereo\|c0=c1\|c1=c0` |
| Stereo → mono downmix | stereo/mono converters | in-model | `operation=mono` → `-ac 1` (folds 5.1/7.1 correctly, not naive L+R) |
| Mono → stereo (duplicate) | up-mixers | in-model | `operation=stereo` → `-ac 2`; already-stereo passes through |
| Copy one side onto both ("fix one earbud") | one-sided-audio fixers | in-model | `operation=left\|right` → `pan=stereo\|c0=cN\|c1=cN` |
| Output format choice | most | in-model | family-standard enum mp3/wav/ogg/flac/m4a, default mp3 |
| Local/in-browser processing | ffmpeg.wasm tools | in-model | how gizza pages work; stated on page |
| Preset chips for common jobs | some | in-model | meta examples: swap, stereo→mono, fix one-sided (left→both) |
| Downmix-law / gain choice (0.5 vs -3 dB) | one desktop tool | out-of-model | ffmpeg standard law only; separate concern |
| Per-channel gain / balance slider | some editors | out-of-model | no meter/slider UI in the page framework; use audio-volume-adjust |
| Mid/side, phase-invert, arbitrary channel matrix | pro plugins | out-of-model | beyond the five common jobs; separate tool if backlog wants it |

## Design decisions

- Five operations cover the common jobs seen across competitors — swap, downmix, up-mix,
  and copy-left/copy-right — from a single ffmpeg pass; `operation` defaults to `swap`.
- `mono` uses `-ac 1` (not a hand-written pan) so multi-channel sources fold down with
  ffmpeg's proper weights; `stereo` uses `-ac 2`. `swap`/`left`/`right` use `pan=stereo`
  and deliberately omit `-ac` because pan already fixes the layout.
- `-vn` drops attached-picture (album-art) streams so channel routing never trips on a
  video/image stream.
- Output format is the family-standard mp3/wav/ogg/flac/m4a enum (lossy at 192 kbps),
  default mp3, matching audio-convert / audio-to-mono for cross-tool consistency.
- Verification proves routing spectrally: the stereo fixture carries 440 Hz on the left and
  880 Hz on the right; the page test decodes the output and counts zero crossings — after
  `swap`, the left channel must land near the 880 Hz rate (proving the sides really moved),
  and `mono` must produce `numberOfChannels === 1`.
