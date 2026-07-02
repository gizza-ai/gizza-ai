# audio-to-mono — competitor analysis (2026-07-02)

One WebSearch ("convert stereo to mono audio online tool downmix"); skimmed the top real
tools: Notevibes stereo-to-mono, WuTools stereo-mono converter, mp3cut.org stereo-to-mono,
AudioAlter downmixer, RouteNote downmixer, omnvert stereo/mono mixer.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| (L+R)/2 downmix of all channels | all | in-model | `channel=mix` default → ffmpeg `-ac 1` (also folds down 5.1/7.1 correctly) |
| Keep only left / only right channel | mp3cut, omnvert, WuTools channel mixer | in-model | `channel=left\|right` → `pan=mono\|c0=c0` / `c0=c1` |
| Local/in-browser processing | Notevibes ("never leaves your device"), WuTools (ffmpeg.wasm) | in-model | how gizza pages work; stated on page |
| Downmix law choice (0.5 vs -3 dB) | WuTools | out-of-model for v1 | ffmpeg's standard law only; listed, not built |
| Phase-correlation / peak meters | WuTools | out-of-model | no meter UI in the page framework |
| L/R swap, mid/side tools | omnvert | out-of-model | separate tool if the backlog wants it |
| Output format choice | most | in-model | family-standard `format` enum, default mp3 |

## Design decisions

- `mix` uses `-ac 1` rather than a hand-written pan formula so multi-channel (5.1/7.1)
  sources fold down with ffmpeg's proper channel weights, not just L/R averaging.
- `left`/`right` use `pan=mono|c0=cN` and deliberately omit `-ac 1` (pan already outputs mono).
- Verification proves channel selection spectrally: the test fixture has a 440 Hz tone on the
  left and 880 Hz on the right; the page test decodes the output and counts zero crossings —
  left-extraction must land near 880 crossings/s, the mix near the blended spectrum, and
  numberOfChannels must be 1. CLI check uses ffprobe channels=1.
