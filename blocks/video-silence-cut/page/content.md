## Cut silent gaps from a video in your browser

Pick a video, set how quiet counts as "silence" (a dB threshold) and the shortest
gap worth cutting (in seconds), and the silent stretches are trimmed out. The
processing runs entirely in your browser with ffmpeg compiled to WebAssembly —
your video is never uploaded to a server.

### Settings

- **Silence threshold (dB)** — audio quieter than this counts as silence. Default
  `-30`. Use a lower value (e.g. `-40`) to only cut near-total silence, or a higher
  one (e.g. `-20`) to also cut quiet ambience.
- **Minimum gap (seconds)** — only silent runs at least this long are trimmed.
  Default `0.5`. Raise it to keep natural short pauses.

### Heads up — this is a single-pass approximation

True jump-cut silence removal (keeping audio **and** video perfectly in sync) is a
two-pass operation: detect the silent timestamps, then cut the matching segments
from both streams. This tool runs in a single ffmpeg pass, which can de-silence the
**audio** but cannot drop the matching **video** frames — so after the first removed
gap the video drifts out of sync with the audio. The output is a valid, shorter
video and the soundtrack is tightened, but the picture timing is approximate. For a
frame-accurate cut, use a desktop editor.

### Tips

- Works offline once the page has loaded.
- Output is always re-encoded to mp4 (h264/aac).
