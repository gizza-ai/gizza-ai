# gif-frame-deduplicator — competitor analysis (2026-09-24)

Search performed before shipping: `online GIF optimizer remove duplicate frames merge frame delays mpdecimate gifsicle` (2026-09-24). The top reachable competitor shapes were: EZGIF's GIF optimizer, FreeConvert's GIF compressor, Online GIF Tools' remove-frame utility, and a smaller GIF optimizer result that explicitly describes dropping identical consecutive frames and extending the kept frame's delay. The notes below paraphrase those observed surfaces and ffmpeg's own `mpdecimate` filter behavior; no competitor copy, branding language or trademarks are reused. Out-of-model items are *listed*, not built.

## Landscape (by category, not by brand)

### K1 — EZGIF-style general-purpose GIF optimizers
The dominant category. They upload the GIF to a server, run a lossy colour/palette pass, offer LZW/colour/frame-rate style reductions, and hand back a smaller GIF. The search result explicitly included a "remove duplicate frames" capability, so duplicate removal is table-stakes, but it appears as one optimizer mode among many rather than as a focused one-knob deduplicator. Typical adjacent controls: compression level, colour count, resize/crop, and frame-rate reduction.

### K2 — FreeConvert-style GIF compressors
These present the job as compression rather than editing. The surfaced description explicitly calls out removing similar/duplicate frames as one way to shrink a GIF, alongside more generic compression options. The relevant table stake is a single user-understandable sensitivity/compression setting instead of raw ffmpeg thresholds.

### K3 — Online GIF Tools-style positional frame removers
This category deletes frames by ordinal pattern, such as all odd/even frames or every Nth frame. It is useful as a manual/regular thinning primitive, but not content-aware: it deletes important frames and keeps duplicate padding if the pattern says so. This is a non-goal for the new tool, except that it confirms users expect frame-level GIF tools to expose simple examples and explain timing consequences.

### K4 — dedicated identical-frame/delay-merging optimizers
One search result's summary matched the exact model: when consecutive frames are identical, drop the duplicates and extend the remaining frame's display delay to preserve playback. That maps directly to ffmpeg `mpdecimate` plus variable-frame-rate output: duplicates disappear, timestamps stay in place, and the surviving frame holds for the gap.

### K5 — `gifsicle` wrappers and CLI recipes
`gifsicle -O3` performs *transparency-based inter-frame optimisation*: it rewrites each
frame to store only the pixels that changed from the previous one. That shrinks a
repetitive GIF a lot, but the frame **count** is unchanged — a truly identical frame
becomes a near-empty frame rather than disappearing. Deleting frames and merging their
delays is a separate manual step (`--delete #n --delay`), which is exactly the job this
tool automates.

### K6 — ffmpeg recipe blogs / Stack Overflow answers
Where the `mpdecimate` approach comes from. The canonical snippet is
`-vf mpdecimate -fps_mode vfr` (older: `-vsync vfr`), plus, for GIF output, the
`split → palettegen → paletteuse` pair. Two failure modes recur in those threads and both
shaped this implementation:
  - **Forgetting the frame-rate mode.** Without `-fps_mode vfr` ffmpeg re-inserts every
    decimated frame to hold a constant rate, and the output is the same size as the input
    — the "mpdecimate did nothing" trap.
  - **Adding `setpts=N/FRAME_RATE/TB` unthinkingly.** That closes the timing gaps too, so
    the animation gets *shorter*, which is a different feature (remove the pauses) and a
    surprising default for a deduplicator.

### K7 — video editors / GIF editors with a frame timeline
Desktop-class tools let you see the frame strip and delete frames by hand, with exact
delay editing. Precise, but linear in the number of frames and hopeless for a 600-frame
screen capture.

## Table stakes → where each one landed

| # | Table stake | Seen in | Verdict | Where it lives |
| --- | --- | --- | --- | --- |
| 1 | Reduce GIF frame count without wrecking the animation | K1, K2, K4, K7 | **in-model** | `mpdecimate` + `-fps_mode vfr` |
| 2 | Content-aware duplicate detection (not "every Nth frame") | K1, K2, K4, K6 | **in-model** — the differentiator | `threshold` → `hi`/`lo` |
| 3 | A single understandable sensitivity knob | K1, K2 | **in-model** | `threshold` 0-100 %, default 98 |
| 4 | Preserve playback duration | K4, K5, K7 | **in-model** | original timestamps kept (VFR), no `setpts` |
| 5 | Keep the GIF looping | K1, K5 | **in-model** | `-loop 0` |
| 6 | Don't degrade colours on re-encode | K1 (palette controls) | **in-model** | `palettegen=stats_mode=diff` + `paletteuse` bayer dither |
| 7 | A stated size limit | K1, K2 | **in-model** | 32 MiB in/out, stated on the page |
| 8 | Local processing / no upload | uncommon in K1/K2 (server-side is normal) | **in-model** | ffmpeg-wasm in the page |
| 9 | Report frames-in → frames-out | K1, K7 | **considered, rejected** | the ffmpeg-runtime dispatch returns output *bytes*, not filter statistics; the chat summary reports byte sizes instead. Counting frames would need a second decode pass. |
| 10 | Colour-count / lossy compression knob | K1, K2, K5 | **out-of-model here** | already covered by the separate `gif-optimize` tool (scale / frame_step / colour bits); duplicating it would blur both tools |
| 11 | Resize while deduplicating | K1, K2 | **out-of-model here** | `gif-resize` / `gif-optimize` own that |
| 12 | Inter-frame transparency optimisation (`gifsicle -O3`) | K5 | **out-of-model** | not something ffmpeg's GIF encoder exposes; it needs `gifsicle`, which isn't in the runtime |
| 13 | Frame-by-frame timeline with manual delete | K3, K7 | **out-of-model** | the page renders a single media output, not an editable frame strip |
| 14 | Batch upload + ZIP download | K1, K2 | **out-of-model** | the page takes one file and multi-input ffmpeg is recorded as un-buildable in this repo |
| 15 | Exact "remove the pauses" (shorten the GIF) | K6 recipes | **considered, rejected** | that is the `setpts` variant; `video-dedup-frames` already exposes it as `timing = compact` for video, and making it the GIF default would silently change animation length |

## Distinctness from the neighbouring tools in this repo

- **`gif-optimize`** is pure-Rust and positional: it resizes, keeps every Nth frame and
  posterizes colours. It cannot tell a duplicate frame from a distinct one.
- **`video-dedup-frames`** does content-aware `mpdecimate` for *video*, with codec, audio
  and CFR/VFR/compact timing choices, and cannot write a GIF.
- This tool is the GIF-shaped intersection: content-aware decimation, GIF in / GIF out,
  one knob, palette-quality re-encode. Kept as a separate tool rather than a flag on
  either neighbour because the parameter sets barely overlap.

## UX control patterns adopted

- **Threshold expressed as similarity %, not as raw `hi`/`lo`.** K1's audience understands
  a percentage slider; nobody outside K3 knows what a sum-of-absolute-differences of 768
  means. The mapping (`hi = 64 × 255 × (100 − threshold)/100`, `lo` at ffmpeg's 12:5
  ratio) is documented in the core module so it stays defensible.
- **Default 98 rather than ffmpeg's own defaults (≈95.3% equivalent).** A GIF tool should
  err towards keeping frames; the page documents 93-97 for screen captures where more
  aggression pays off.
- **The page states the trap the K3 recipes fall into** ("will the GIF play faster?"), as
  the first FAQ entry, because that is the question a user of any of these tools asks
  first.
- **Input restricted to `.gif`** with a named error, instead of silently accepting a PNG
  and emitting a one-frame GIF.
