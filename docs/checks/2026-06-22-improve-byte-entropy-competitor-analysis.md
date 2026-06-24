# byte-entropy — competitor analysis (2026-06-22)

## Tool

`gizza-ai/byte-entropy` — computes the Shannon entropy (bits/byte, 0–8) of a
file, both overall and across fixed-size blocks, to spot encrypted, compressed,
or packed regions. Surfaces: **chat + CLI** (no standalone page — a file→JSON
report fits neither the pure-text page nor the ffmpeg file→media page shape;
same "no-page file-input" pattern as `detect-file-type` / `web-fetch`).

## Surfaces verified

- **Chat**: `wafer build` validates `target/block.wasm` instantiates (pure Rust,
  runs on every backend including the chat Service Worker). 491.9 KiB.
- **CLI**: `gizza tool byte-entropy url=… [block_size=N]` — verified on a small
  zip (`overall_entropy 4.75`, "moderate") and a PNG (`overall_entropy 7.17`,
  "high", 2 blocks with a min/max bracket of 6.57–6.77). `gizza list` shows it.
- **Page**: N/A (no-page file-input tool).
- Unit tests: 8 core + 3 block (incl. the drift-guard `schema_json` test). All pass.

## Competitors surveyed (top reference points)

1. **binwalk** (ReFirmLabs) — the de-facto firmware-analysis tool; its
   `-E`/`--entropy` mode plots Shannon entropy across a file to find
   compressed/encrypted regions, and flags "rising/falling entropy edges".
2. **CyberChef** "Entropy" operation (GCHQ) — computes Shannon entropy of the
   input and renders an entropy curve (scan window). Browser-based, no upload.
3. **ent** (Fourmilab) — classic CLI: entropy in bits/byte, chi-square,
   arithmetic mean, Monte-Carlo π, serial-correlation coefficient.
4. **Detect It Easy (DiE)** — PE/binary inspector with an entropy panel per
   section, highlighting packed/encrypted sections (>7.0 ≈ packed).
5. **`scipy.stats.entropy` / numpy one-liners** — the common "roll your own"
   reference; just the overall value, no blocking, no assessment.

## Capability diff and gap ranking (fit-to-model)

| Capability | Competitors | byte-entropy | Decision |
|---|---|---|---|
| Overall Shannon entropy (bits/byte) | all | yes | covered |
| Per-block / windowed entropy series | binwalk, CyberChef, DiE | **yes** (`block_size`, `blocks[]`) | covered |
| Min/max block entropy + offsets | binwalk | **yes** | covered |
| Distinct-byte count | ent (implicit) | **yes** | covered (added) |
| Plain-English assessment (encrypted/compressed/text) | DiE/binwalk imply via thresholds | **yes** (`assessment`) | covered (added) |
| Configurable window size | binwalk, CyberChef | **yes** (16–4 MiB, default 256) | covered |
| Entropy **graph/plot** image | binwalk, CyberChef, DiE | no | **out of model** — image-bytes output has no page render mode here; JSON `blocks[]` series is the chartable substitute, and the chat client/LLM can render it. Listed, not built. |
| Chi-square / arithmetic-mean / serial-correlation (ent's extra stats) | ent | no | **deferred** — separate statistical-randomness-test tool, out of this tool's "entropy" scope; would belong in its own block. |
| PE/ELF **section-aware** entropy | DiE, binwalk | no | **out of model** — needs a full executable-format parser; distinct tool. The generic byte/block view still works on any file. |
| Magic-byte file-type ID | binwalk, DiE | already a sibling tool | covered by `detect-file-type` (no dup). |

## Gaps closed this pass

Beyond the raw overall+per-block entropy, added during the build to match
competitor depth that *is* in-model: `distinct_bytes`, the `min/max_block_entropy`
bracket with per-block `offset`/`size` (the binwalk "where are the high-entropy
regions" use case), and a human-readable `assessment` mapping the 0–8 range to
encrypted/compressed/media/text/padding — so the LLM and CLI user get a verdict,
not just a number. `block_size` is clamped (16 B – 4 MiB) to keep the `blocks[]`
array bounded.

## Out-of-model (not built — by design)

- Rendered entropy **plot/curve** image (no image-page render mode; series is
  returned as JSON instead).
- Extra randomness statistics (chi-square, Monte-Carlo π, serial correlation) —
  separate tool scope.
- Executable section-aware entropy (needs PE/ELF parsing) — separate tool.

No competitor copy, branding, or trademarks were used.
