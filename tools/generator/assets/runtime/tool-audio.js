// Shared interactive audio waveform for tool pages (audio-input ffmpeg tools
// and audio outputs). Dependency-free: WebAudio decode → canvas peaks, a
// hidden <audio> element for playback, Pointer Events for seek/select.
// Loaded lazily by tool.js; every failure path leaves the page's normal
// flow untouched (this component is an enhancement, never a gate).

const HEIGHT = 88;       // waveform lane height (CSS px, fixed — no layout jump)
const CLICK_PX = 4;      // movement under this = click (seek), not drag
const HANDLE_PX = 8;     // hit zone around a selection edge
const MIN_SEL_S = 0.05;  // selection edges can't cross closer than this
const BASE_COLS = 4096;  // fixed-resolution peak cache (~32 KB per widget)

function fmtTime(t) {
  if (!Number.isFinite(t) || t < 0) t = 0;
  const m = Math.floor(t / 60);
  const s = t - m * 60;
  return `${m}:${s < 10 ? "0" : ""}${s.toFixed(1)}`;
}

export function createWaveform(container, opts = {}) {
  const interactive = opts.interactive !== false;
  const binding = opts.selection || null;

  container.classList.add("tool-wf");
  container.innerHTML = "";
  const wave = document.createElement("div");
  wave.className = "tool-wf-wave";
  wave.tabIndex = 0;
  wave.setAttribute("role", "group");
  wave.setAttribute(
    "aria-label",
    interactive
      ? "Audio waveform. Click to seek, drag to select. Space plays or pauses, arrow keys seek."
      : "Result waveform. Click to seek. Space plays or pauses, arrow keys seek."
  );
  const canvas = document.createElement("canvas");
  canvas.className = "tool-wf-canvas";
  wave.appendChild(canvas);
  const ctx = canvas.getContext("2d");
  // The static gray waveform envelope depends only on `peaks` (recomputed
  // solely in resize()). Rasterize it ONCE per resize into this offscreen
  // canvas; draw() then blits the bitmap each frame (O(1)) instead of
  // re-drawing ~clientWidth fillRects (O(width)) per animation frame.
  const envelopeCanvas = document.createElement("canvas");
  const envelopeCtx = envelopeCanvas.getContext("2d");

  const bar = document.createElement("div");
  bar.className = "tool-wf-bar";
  const playBtn = document.createElement("button");
  playBtn.type = "button";
  playBtn.className = "tool-wf-btn tool-wf-play";
  playBtn.textContent = "Play";
  const playSelBtn = document.createElement("button");
  playSelBtn.type = "button";
  playSelBtn.className = "tool-wf-btn tool-wf-playsel";
  playSelBtn.textContent = "Play selection";
  playSelBtn.hidden = true;
  const timeEl = document.createElement("span");
  timeEl.className = "tool-wf-time";
  bar.append(playBtn, playSelBtn, timeEl);
  container.append(wave, bar);
  container.hidden = true;

  const audioEl = new Audio();
  audioEl.preload = "metadata";

  // The decoded AudioBuffer is NOT retained (a 10 MiB mp3 decodes to
  // ~150 MB of PCM — prohibitive on mobile). load() reduces it once to a
  // fixed-resolution peak cache; resize only resamples that.
  let basePeaks = null; // Float32Array, [min,max] × BASE_COLS; null = no audio loaded
  let loadSeq = 0; // load() generation — a newer load()/clear() invalidates in-flight loads
  let duration = 0;
  let objectUrl = null;
  let peaks = null; // Float32Array, [min,max] per canvas-css-px column
  let sel = null;   // {start, end} seconds, or null
  let playScope = null; // "all" | "selection" while playing
  let rafId = 0;
  let cssWidth = 0; // cached wave.clientWidth (CSS px); refreshed in resize()

  const xOf = (t) => (duration ? (t / duration) * cssWidth : 0);
  const tOf = (x) =>
    Math.max(0, Math.min(duration, (x / Math.max(1, cssWidth)) * duration));

  // Clamp a raw bounds pair to [0, duration]. THE bounds interpreter — call
  // sites hand over raw field values and must not pre-map them.
  function normalizeSel(start, end) {
    const s = Math.max(0, Math.min(duration, Number(start) || 0));
    // Empty, 0, negative or non-numeric end = unbounded ("to the end of the
    // track" — matches the tools' end-sentinel semantics).
    const eRaw = end == null || end === "" ? NaN : Number(end);
    const e =
      Number.isFinite(eRaw) && eRaw > 0
        ? Math.max(s + MIN_SEL_S, Math.min(duration, eRaw))
        : duration;
    // Exactly-whole-track bounds mean "no selection" (so drag-to-select works
    // at bound-field defaults). Float-noise epsilon only — a real 0.04 s inset
    // selection must still draw its highlight.
    return s > 1e-9 || e < duration - 1e-9 ? { start: s, end: e } : null;
  }

  // One-time reduction of the decoded buffer to `width` [min,max] columns —
  // the only code that touches PCM. Called once per load(); the buffer is
  // dropped afterwards.
  function computePeaks(buffer, width) {
    const chans = [];
    for (let c = 0; c < buffer.numberOfChannels; c++) {
      chans.push(buffer.getChannelData(c));
    }
    const per = buffer.length / width;
    const out = new Float32Array(width * 2);
    for (let x = 0; x < width; x++) {
      let mn = 1, mx = -1;
      const s = Math.floor(x * per);
      const e = Math.min(buffer.length, Math.ceil((x + 1) * per));
      const step = Math.max(1, Math.floor((e - s) / 512));
      for (const data of chans) {
        for (let i = s; i < e; i += step) {
          const v = data[i];
          if (v < mn) mn = v;
          if (v > mx) mx = v;
        }
      }
      out[x * 2] = mn > mx ? 0 : mn;
      out[x * 2 + 1] = mn > mx ? 0 : mx;
    }
    return out;
  }

  // Resample the fixed-resolution cache to display width: min-of-mins /
  // max-of-maxes over each column's base range, so the envelope stays exact.
  // For x < width: s ≤ cols-1 and e ≥ s+1 (the quotients are ≥ cols/width
  // apart, far above float noise), and base pairs are always ordered
  // (computePeaks writes 0,0 for empty windows) — every column folds at
  // least one real pair, so no emptiness guards are needed here.
  function resamplePeaks(width) {
    const cols = basePeaks.length / 2;
    const out = new Float32Array(width * 2);
    for (let x = 0; x < width; x++) {
      const s = Math.floor((x * cols) / width);
      const e = Math.ceil(((x + 1) * cols) / width);
      let mn = 1, mx = -1;
      for (let i = s; i < e; i++) {
        if (basePeaks[i * 2] < mn) mn = basePeaks[i * 2];
        if (basePeaks[i * 2 + 1] > mx) mx = basePeaks[i * 2 + 1];
      }
      out[x * 2] = mn;
      out[x * 2 + 1] = mx;
    }
    return out;
  }

  // Rasterize the static gray envelope into the offscreen canvas. Rendered
  // with the SAME setTransform(dpr,…) + CSS-px coords the old inline loop
  // used, so the cached bitmap is crisp at any DPR. Called once per resize().
  function renderEnvelope(dpr, w, h) {
    envelopeCanvas.width = w;
    envelopeCanvas.height = h;
    const g = envelopeCtx;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, cssWidth, HEIGHT);
    g.fillStyle = "#64748b";
    const mid = HEIGHT / 2;
    for (let x = 0; x < cssWidth && x * 2 + 1 < peaks.length; x++) {
      const y1 = mid + peaks[x * 2] * (mid - 2);
      const y2 = mid + peaks[x * 2 + 1] * (mid - 2);
      g.fillRect(x, Math.min(y1, y2), 1, Math.max(1, Math.abs(y2 - y1)));
    }
  }

  function draw() {
    if (!peaks) return;
    const dpr = window.devicePixelRatio || 1;
    const w = cssWidth;
    const h = HEIGHT;
    const g = ctx;
    // The translucent selection fill sits BEHIND the bars (as in the original
    // draw order), so paint it first in CSS coords, then blit the envelope
    // over it: the envelope's opaque bars cover the tint while its transparent
    // gaps let the tint show through — source-over is associative, so this is
    // pixel-identical to the old fill→bars→markers→playhead sequence.
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    if (sel) {
      g.fillStyle = "rgba(37, 99, 235, 0.15)";
      g.fillRect(xOf(sel.start), 0, xOf(sel.end) - xOf(sel.start), h);
    }
    // Blit the pre-rasterized envelope at device px, 1:1 (identity transform).
    g.setTransform(1, 0, 0, 1, 0, 0);
    g.drawImage(envelopeCanvas, 0, 0);
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (sel) {
      g.fillStyle = "#2563eb";
      g.fillRect(xOf(sel.start) - 1.5, 0, 3, h);
      g.fillRect(xOf(sel.end) - 1.5, 0, 3, h);
    }
    g.fillStyle = "#0f172a";
    g.fillRect(xOf(audioEl.currentTime || 0) - 0.5, 0, 1, h);
  }

  // updateBar runs every rAF (~60 fps) but the 0.1 s-resolution readout and
  // the button states change ≤10×/s — track the last written values and only
  // touch the DOM on a real change. `updateBar` is the sole writer of these
  // three properties, so the cached values stay authoritative.
  let lastBarText = null;
  let lastPlayLabel = null;
  let lastPlaySelHidden = null;
  function updateBar() {
    let t = `${fmtTime(audioEl.currentTime)} / ${fmtTime(duration)}`;
    if (sel) {
      t += ` · ${fmtTime(sel.start)}–${fmtTime(sel.end)} (${(sel.end - sel.start).toFixed(1)}s)`;
    }
    if (t !== lastBarText) {
      timeEl.textContent = t;
      lastBarText = t;
    }
    const playSelHidden = !sel;
    if (playSelHidden !== lastPlaySelHidden) {
      playSelBtn.hidden = playSelHidden;
      lastPlaySelHidden = playSelHidden;
    }
    const playLabel = audioEl.paused ? "Play" : "Pause";
    if (playLabel !== lastPlayLabel) {
      playBtn.textContent = playLabel;
      lastPlayLabel = playLabel;
    }
  }

  let peaksFrom = null; // the basePeaks the current `peaks` were resampled from
  function resize() {
    if (!basePeaks || wave.clientWidth === 0) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.round(wave.clientWidth * dpr);
    const h = Math.round(HEIGHT * dpr);
    // Hidden→visible loads fire the ResizeObserver right after load()'s
    // explicit resize() call — skip the duplicate resample when nothing
    // changed. (The explicit call stays: a same-width reload never fires RO.)
    if (canvas.width === w && canvas.height === h && peaksFrom === basePeaks) return;
    cssWidth = wave.clientWidth;
    canvas.width = w;
    canvas.height = h;
    peaks = resamplePeaks(cssWidth);
    peaksFrom = basePeaks;
    renderEnvelope(dpr, w, h);
    draw();
  }
  const ro = new ResizeObserver(resize);
  ro.observe(wave);

  function tick() {
    if (playScope === "selection" && sel && audioEl.currentTime >= sel.end) {
      audioEl.pause();
      audioEl.currentTime = sel.end;
    }
    draw();
    updateBar();
    if (!audioEl.paused) rafId = requestAnimationFrame(tick);
  }
  audioEl.addEventListener("play", () => {
    cancelAnimationFrame(rafId);
    rafId = requestAnimationFrame(tick);
  });
  audioEl.addEventListener("pause", () => {
    cancelAnimationFrame(rafId);
    playScope = null;
    draw();
    updateBar();
  });
  audioEl.addEventListener("ended", () => {
    playScope = null;
    draw();
    updateBar();
  });

  playBtn.addEventListener("click", () => {
    if (audioEl.paused) {
      playScope = "all";
      audioEl.play();
    } else {
      audioEl.pause();
    }
  });
  playSelBtn.addEventListener("click", () => {
    if (!sel) return;
    audioEl.currentTime = sel.start;
    playScope = "selection";
    audioEl.play();
  });

  wave.addEventListener("keydown", (e) => {
    if (e.key === " ") {
      e.preventDefault();
      playBtn.click();
    } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      const d = e.key === "ArrowLeft" ? -1 : 1;
      audioEl.currentTime = Math.max(0, Math.min(duration, audioEl.currentTime + d));
      draw();
      updateBar();
    }
  });

  // Pointer interaction: click = seek; drag = create/resize/move selection.
  let ptr = null; // {x0, mode, startSel}
  function modeAt(x) {
    if (!interactive) return "seek";
    if (sel) {
      if (Math.abs(x - xOf(sel.start)) <= HANDLE_PX) return "resize-start";
      if (Math.abs(x - xOf(sel.end)) <= HANDLE_PX) return "resize-end";
      if (x > xOf(sel.start) && x < xOf(sel.end)) return "move";
    }
    return "create";
  }
  wave.addEventListener("pointerdown", (e) => {
    if (!basePeaks || e.button !== 0) return;
    wave.focus({ preventScroll: true });
    wave.setPointerCapture(e.pointerId);
    const x = e.offsetX;
    ptr = { x0: x, moved: false, mode: modeAt(x), startSel: sel ? { ...sel } : null };
  });
  wave.addEventListener("pointermove", (e) => {
    if (!ptr) return;
    const x = e.offsetX;
    if (!ptr.moved && Math.abs(x - ptr.x0) < CLICK_PX) return;
    ptr.moved = true;
    const t = tOf(x);
    if (ptr.mode === "create") {
      const a = tOf(ptr.x0);
      sel = { start: Math.min(a, t), end: Math.max(a, t) };
    } else if (ptr.mode === "resize-start" && sel) {
      sel = { start: Math.min(t, sel.end - MIN_SEL_S), end: sel.end };
      if (sel.start < 0) sel.start = 0;
    } else if (ptr.mode === "resize-end" && sel) {
      sel = { start: sel.start, end: Math.max(t, sel.start + MIN_SEL_S) };
      if (sel.end > duration) sel.end = duration;
    } else if (ptr.mode === "move" && ptr.startSel) {
      const len = ptr.startSel.end - ptr.startSel.start;
      let s = ptr.startSel.start + (t - tOf(ptr.x0));
      s = Math.max(0, Math.min(duration - len, s));
      sel = { start: s, end: s + len };
    } else {
      return; // "seek" mode: no drag behavior on output views
    }
    if (sel && binding) binding.onDrag(sel.start, sel.end);
    draw();
    updateBar();
  });
  wave.addEventListener("pointerup", (e) => {
    if (!ptr) return;
    const wasDrag = ptr.moved;
    const mode = ptr.mode;
    ptr = null;
    if (!wasDrag || mode === "seek") {
      // Click. Audition-only tools: clicking outside the selection clears it.
      const t = tOf(e.offsetX);
      if (!binding && interactive && sel && (t < sel.start || t > sel.end) && mode === "create") {
        sel = null;
      }
      audioEl.currentTime = t;
      draw();
      updateBar();
      return;
    }
    if (sel && binding) binding.onCommit(sel.start, sel.end);
    updateBar();
  });
  wave.addEventListener("pointercancel", () => {
    ptr = null;
  });

  function revoke() {
    if (objectUrl) {
      URL.revokeObjectURL(objectUrl);
      objectUrl = null;
    }
  }

  return {
    async load(blob) {
      // Callers fire-and-forget load() (file-input change handlers), so
      // overlapping loads can decode out of order — only the newest may
      // commit state, or a slow old file overwrites a fast new one and
      // orphans its object URL.
      const seq = ++loadSeq;
      audioEl.pause();
      revoke();
      let buffer;
      try {
        const raw = await blob.arrayBuffer();
        const Ctx = window.AudioContext || window.webkitAudioContext;
        const ctx = new Ctx();
        try {
          // decodeAudioData detaches `raw`; fine — it has no later use.
          buffer = await ctx.decodeAudioData(raw);
        } finally {
          ctx.close();
        }
      } catch (err) {
        if (seq !== loadSeq) return false;
        basePeaks = null;
        container.hidden = true;
        return false;
      }
      if (seq !== loadSeq) return false; // superseded by a newer load/clear
      if (!buffer.length) {
        // Zero PCM frames can decode "successfully" on some platforms —
        // treat it as a failed load, not a loaded-but-empty widget.
        basePeaks = null;
        container.hidden = true;
        return false;
      }
      duration = buffer.duration;
      // Reduce to the peak cache and let the PCM go out of scope.
      basePeaks = computePeaks(buffer, Math.min(BASE_COLS, buffer.length));
      objectUrl = URL.createObjectURL(blob);
      audioEl.src = objectUrl;
      sel = null;
      if (binding) {
        const b = binding.getBounds();
        sel = normalizeSel(b.start, b.end);
      }
      container.hidden = false;
      resize();
      updateBar();
      return true;
    },
    setSelection(start, end) {
      if (!basePeaks) return;
      sel = normalizeSel(start, end);
      draw();
      updateBar();
    },
    clear() {
      loadSeq++; // an in-flight load() must not resurrect a cleared widget
      audioEl.pause();
      revoke();
      basePeaks = null;
      peaks = null;
      sel = null;
      container.hidden = true;
    },
    destroy() {
      loadSeq++;
      cancelAnimationFrame(rafId);
      ro.disconnect();
      audioEl.pause();
      revoke();
      container.innerHTML = "";
      container.hidden = true;
    },
  };
}
