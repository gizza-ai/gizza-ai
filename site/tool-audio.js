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
  let duration = 0;
  let objectUrl = null;
  let peaks = null; // Float32Array, [min,max] per canvas-css-px column
  let sel = null;   // {start, end} seconds, or null
  let playScope = null; // "all" | "selection" while playing
  let rafId = 0;

  const xOf = (t) => (duration ? (t / duration) * wave.clientWidth : 0);
  const tOf = (x) =>
    Math.max(0, Math.min(duration, (x / Math.max(1, wave.clientWidth)) * duration));

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
  function resamplePeaks(width) {
    const cols = basePeaks.length / 2;
    const out = new Float32Array(width * 2);
    for (let x = 0; x < width; x++) {
      const s = Math.min(cols - 1, Math.floor((x * cols) / width));
      const e = Math.max(s + 1, Math.min(cols, Math.ceil(((x + 1) * cols) / width)));
      let mn = 1, mx = -1;
      for (let i = s; i < e; i++) {
        if (basePeaks[i * 2] < mn) mn = basePeaks[i * 2];
        if (basePeaks[i * 2 + 1] > mx) mx = basePeaks[i * 2 + 1];
      }
      out[x * 2] = mn > mx ? 0 : mn;
      out[x * 2 + 1] = mn > mx ? 0 : mx;
    }
    return out;
  }

  function draw() {
    if (!peaks) return;
    const dpr = window.devicePixelRatio || 1;
    const w = wave.clientWidth;
    const h = HEIGHT;
    const g = canvas.getContext("2d");
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    if (sel) {
      g.fillStyle = "rgba(37, 99, 235, 0.15)";
      g.fillRect(xOf(sel.start), 0, xOf(sel.end) - xOf(sel.start), h);
    }
    g.fillStyle = "#64748b";
    const mid = h / 2;
    for (let x = 0; x < w && x * 2 + 1 < peaks.length; x++) {
      const y1 = mid + peaks[x * 2] * (mid - 2);
      const y2 = mid + peaks[x * 2 + 1] * (mid - 2);
      g.fillRect(x, Math.min(y1, y2), 1, Math.max(1, Math.abs(y2 - y1)));
    }
    if (sel) {
      g.fillStyle = "#2563eb";
      g.fillRect(xOf(sel.start) - 1.5, 0, 3, h);
      g.fillRect(xOf(sel.end) - 1.5, 0, 3, h);
    }
    g.fillStyle = "#0f172a";
    g.fillRect(xOf(audioEl.currentTime || 0) - 0.5, 0, 1, h);
  }

  function updateBar() {
    let t = `${fmtTime(audioEl.currentTime)} / ${fmtTime(duration)}`;
    if (sel) {
      t += ` · ${fmtTime(sel.start)}–${fmtTime(sel.end)} (${(sel.end - sel.start).toFixed(1)}s)`;
    }
    timeEl.textContent = t;
    playSelBtn.hidden = !sel;
    playBtn.textContent = audioEl.paused ? "Play" : "Pause";
  }

  function resize() {
    if (!basePeaks || wave.clientWidth === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(wave.clientWidth * dpr);
    canvas.height = Math.round(HEIGHT * dpr);
    peaks = resamplePeaks(wave.clientWidth);
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
      audioEl.pause();
      revoke();
      let buffer;
      try {
        const raw = await blob.arrayBuffer();
        const Ctx = window.AudioContext || window.webkitAudioContext;
        const ctx = new Ctx();
        try {
          // decodeAudioData detaches its buffer — hand it a copy.
          buffer = await ctx.decodeAudioData(raw.slice(0));
        } finally {
          ctx.close();
        }
      } catch (err) {
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
      audioEl.pause();
      revoke();
      basePeaks = null;
      peaks = null;
      sel = null;
      container.hidden = true;
    },
    destroy() {
      cancelAnimationFrame(rafId);
      ro.disconnect();
      audioEl.pause();
      revoke();
      container.innerHTML = "";
      container.hidden = true;
    },
  };
}
