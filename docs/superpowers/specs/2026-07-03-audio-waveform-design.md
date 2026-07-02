# Interactive audio waveform for tool pages — design

**Date:** 2026-07-03 · **Status:** approved by user (chat), pending spec review
**Branch:** `audio-waveform` (off `tool-loop/2026-07-02`; the 10 audio tools it enhances live there until PR #160 merges)

## Goal

Every audio tool page should let the user *see* their audio and work with it visually:
scrub, play, audition any section, and — where the tool takes a time range — select that
range by dragging on the waveform. Both the uploaded file and the tool's result render as
waveforms so before/after is visible (silence removed, fade ramps, gain changes).

## User experience

After choosing a file on any audio-input tool page (`accept="audio/*"`, runtime `ffmpeg`):

- A waveform of the whole track renders below the file picker.
- Click anywhere → playhead moves there. Play/pause button; space bar while the widget has
  focus; arrow keys seek ±1 s.
- Drag horizontally → a shaded selection with edge handles. "Play selection" plays exactly
  that region and stops. Dragging an edge handle adjusts it; dragging the middle moves it.
  On audition-only tools a click outside the selection clears it; on a field-bound tool
  (trim-audio) the selection always mirrors the fields and is never "cleared" — a plain
  click just moves the playhead.
- Time readout: `current / total`, plus `start – end (length)` while a selection exists.
- **trim-audio only (declared, not hardcoded):** the selection is two-way bound to the
  `start`/`end` fields. Dragging updates the fields live and fires the ffmpeg re-run **on
  release** (drag-end), exactly like image-crop's rectangle. Typing in the fields moves the
  highlight live. An empty `end` field means "to the end of the track".
- On every other audio tool the selection is audition-only — it writes no fields.
- The **output** (`format = "audio"`) renders as a read-only waveform player (seek + play,
  no selection) with the existing Download link. The native `<audio controls>` element
  remains in the DOM as the decode-failure fallback.

## Architecture

One new shared, dependency-free module plus small wiring changes. No per-tool JS.

### `site/tool-audio.js` (new, ~350 lines)

```
createWaveform(container, {
  interactive: true,          // false = output view (seek/play only)
  selection: null | {
    getBounds(),              // read bound fields → {start, end|null} (seconds)
    onDrag(start, end),       // live: write fields, NO run
    onCommit(start, end),     // pointer release: dispatch change → run()
  },
}) → { load(file|blob), setSelection(start, end), destroy() }
```

- Decode: `file.arrayBuffer()` → **copy** → `AudioContext.decodeAudioData` (the API
  detaches its input buffer). Context is closed after decode; decoding needs no user
  gesture (no autoplay-policy dependency).
- Draw: per-pixel min/max peaks over all channels (single pass), `<canvas>` scaled by
  `devicePixelRatio`; redraw on container resize (`ResizeObserver`).
- Playback: hidden `<audio>` element with `URL.createObjectURL` (revoked on reload/
  destroy/Reset). Playhead overlay driven by `requestAnimationFrame` while playing.
  "Play selection" pauses at the selection end (rAF boundary check).
- Input: Pointer Events with `setPointerCapture` — one path for mouse/touch/pen.
  Click vs drag disambiguated by a small movement threshold (~4 px).
- Accessibility: the widget is focusable (`tabindex="0"`, `role="group"`,
  `aria-label`); buttons are real `<button>`s; space/arrows handled only while focused
  (`preventDefault` there and nowhere else).

### `site/tool.js` (ffmpeg path, small addition)

- If the file input's `accept` starts with `audio/` → create the input-waveform container
  and `wf.load(file)` on every file change (also the `?url=` deep-link path). All 10
  existing audio tools — and any future one — get this with zero per-tool code.
- If `cfg.waveform` binding exists → build the `selection` adapter over the named field
  elements: `onDrag` writes field values (1-decimal seconds); `onCommit` dispatches
  `change` so the existing listener runs ffmpeg once. Field `input` events update the
  highlight via `setSelection` (two-way). Binding never writes fields at load time —
  deep-linked/prefilled values are read, not clobbered.
- On a successful run with `format = "audio"` → load the result data-URL into an output
  waveform (read-only); on its decode failure, un-hide the native `<audio>` as today.
- Reset: destroy both waveforms, revoke object URLs, restore the empty state.
- The invariant at the top of `tool.js` stands: no slug branches; behavior is driven by
  `accept` + baked meta.

### meta.toml / generator

New optional table, baked into `window.GIZZA_TOOL` as `cfg.waveform`:

```toml
[waveform]
start = "start"   # field name receiving selection start (seconds)
end   = "end"     # field name receiving selection end
```

Only `blocks/trim-audio/page/meta.toml` declares it today. The generator also bakes each
file input's `accept` into `cfg.inputs` (needed for the audio/* detection). Opt-out flag
(`waveform = false`, top level) for a hypothetical audio tool where the player makes no
sense — no current user.

### CSS

`site/tool.css` additions (~80 lines): container, canvas, playhead, selection shade +
handles, transport row, time readout. Fixed component height (no layout jump after the
initial appearance); responsive width.

## Error handling & edges

- **Decode failure** (exotic codec the browser can't decode even though ffmpeg can):
  waveform hides, a quiet note is NOT shown (silence is fine), the tool runs exactly as
  today. The waveform must never block or delay `run()`.
- Selection clamped to `[0, duration]`; handles can't cross (min 0.05 s).
- trim-audio `mode=remove` uses the same single-selection visual; the page copy explains
  the shading is "the range the fields describe".
- Long files: 10 MiB cap ⇒ ≤ ~7 min mp3; single-pass peak extraction is comfortably fast.
  No zoom in v1.
- Stereo/multichannel: peaks are the max across channels (one lane).

## Testing

New `tests/tool-page-audio-waveform.spec.ts` (runs on trim-audio + one unbound tool):

1. Upload fixture → canvas visible and **non-blank** (sample pixels via `toDataURL`).
2. Pointer-drag a selection → `start`/`end` fields hold the expected seconds → the run
   fires once on release → output's **decoded duration matches the selection** (WebAudio).
3. Typing in `start`/`end` moves the selection overlay (geometry assertion).
4. Play button advances `audio.currentTime`; play-selection stops at the selection end.
5. On an unbound tool (audio-convert): waveform renders, dragging changes **no** fields.
6. Output waveform renders after a run; Download link still works.

All 10 existing audio specs must pass unchanged (they prefill fields and upload — binding
must not clobber those values). Hygiene + generator gates unchanged.

## Out of scope (follow-ups)

- Zoom/scroll for long files.
- Fade-ramp overlay for audio-fade; silence-region preview for audio-silence-remove.
- Video timeline scrubber (separate component; same declarative pattern).

## Rollout

One PR: `site/tool-audio.js`, `tool.css`, `tool.js` wiring, generator baking (`accept`,
`waveform`), trim-audio meta, new spec, regenerated pages. Target: `tool-loop/2026-07-02`
if PR #160 is still open, else `main` after rebase.
