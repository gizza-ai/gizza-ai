## About this tool

Turn a video into a shareable animated GIF without leaving your browser. Pick a
video, choose the **section** you want (a start time and a duration), set the
**frame rate** and **width**, and you get a looping GIF back — the file never
leaves your device.

## How it works

The tool runs ffmpeg, compiled to WebAssembly, with a two-stage palette filter.
First it generates a colour **palette tuned to your exact clip**
(`palettegen`), then it applies that palette to the frames (`paletteuse` with
dithering). That produces a much cleaner GIF at a much smaller size than the
naive fixed-256-colour conversion most converters use.

## Tips for a good GIF

- **Keep it short.** GIFs grow fast — a few seconds is usually plenty. Use the
  **duration** field to trim the clip.
- **Lower the frame rate.** 12 fps (the default) looks smooth for most clips and
  is a fraction of the size of a 30 fps GIF. Drop it to 8–10 fps for big size
  savings.
- **Scale it down.** Set a **width** (height follows automatically to keep the
  aspect ratio) — 320–480 px is great for chat and social. Leave it at 0 to keep
  the source size.
- The GIF **loops forever** by default.

## Notes

- **Private by design.** Everything runs in your browser. No upload, no server.
- Works offline once the page has loaded.
- Very large or very long videos can be slow or memory-heavy in the browser —
  trim to the section you need and pick a sensible width.
