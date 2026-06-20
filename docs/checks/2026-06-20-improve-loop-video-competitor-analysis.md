# loop-video — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/loop-video` — repeat a video/GIF a set number of times or to a
target duration, in one continuous file. Page + CLI (ffmpeg runtime; chat
registers the schema but ffmpeg can't run in the chat Service Worker).

## What competitors do

- **Online video loopers** (kapwing, veed, clideo, ezgif loop) — upload, set
  loops/duration, download. Strengths: simple. Weaknesses: the file is
  **uploaded to a server** (privacy, size caps, queues), watermarks/paywalls on
  free tiers, and several **re-encode** (quality loss + slow) even though looping
  needs no re-encode.
- **`ffmpeg -stream_loop`** — the canonical local approach, but requires the CLI
  and the right flag order.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** ffmpeg-wasm on the page, headless via the
   CLI. The video never leaves the device.
2. **No re-encode.** Uses `-stream_loop` + `-c copy`, so looping is fast and
   **lossless** — the output keeps the exact original quality and container
   (mp4/webm/gif), unlike re-encoding loopers.
3. **Two modes.** Repeat a **count** of times (total plays, 1–100) or loop to a
   **target duration** in seconds (duration takes precedence) — covering both
   "play it 5×" and "make me a 30s loop".
4. **Correct flag handling.** `-stream_loop` is emitted as an input option
   (before `-i`), and `count` is translated to ffmpeg's extra-loops semantics
   (count − 1) so "3 times" really means three plays.
5. **Chainable + guard-railed.** url/ref input, media-envelope output (a `ref`),
   with count and duration caps.

## Honest scope

- Stream-copy requires a loop-friendly container (mp4/webm/mkv/gif all work);
  output keeps the input format (no transcode option here — use video-transcode
  first if needed).
- Duration mode trims at the target (the final repeat may be cut mid-clip, as
  expected for "loop to N seconds").

## Tests

4 core unit tests: count mode sets `-stream_loop (count-1)` before `-i` with
`-c copy` and no `-t`; duration mode uses `-stream_loop -1` + `-t`; output keeps
the input extension; error cases (count 0 / over max / duration over max). Plus
the block drift-guard schema test. CLI verified over the wire on a real clip
(looped count=3 → output ~3× the source duration via ffprobe); Playwright loops
the tiny fixture on the page and asserts a `data:video/…` output — see commit.
