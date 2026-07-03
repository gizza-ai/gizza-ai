# Interactive Audio Waveform Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every audio tool page renders an interactive waveform of the uploaded file (seek, play, audition a dragged selection) and of the result; trim-audio's selection is two-way bound to its `start`/`end` fields.

**Architecture:** One dependency-free shared module `site/tool-audio.js` (WebAudio decode → canvas peaks, `<audio>`-element playback, Pointer Events). `site/tool.js`'s ffmpeg path auto-creates the widget for any tool whose file input accepts `audio/*`; field binding is declared in meta.toml and baked into `window.GIZZA_TOOL` by the Rust page generator. Spec: `docs/superpowers/specs/2026-07-03-audio-waveform-design.md`.

**Tech Stack:** Vanilla ES modules, WebAudio API, Canvas 2D, Pointer Events, Rust (maud/serde) page generator, Playwright.

## Global Constraints

- Zero JS dependencies; no CDN libraries.
- No per-tool slug branches in `site/tool.js` (header invariant) — behavior keys off `accept` + baked meta only.
- The waveform is an enhancement: any failure (decode, module load) must leave today's flow fully working. Wrap wiring in try/catch; never block `run()`.
- ffmpeg runs fire on drag **release** (one `change` event), never per drag-move.
- `decodeAudioData` gets a **copy** (`raw.slice(0)`); every `URL.createObjectURL` is revoked on reload/reset/destroy; the decode `AudioContext` is closed after use.
- Pointer Events + `setPointerCapture` (no mouse/touch dual listeners).
- Keyboard (space/arrows) only while the widget has focus; `preventDefault` there only.
- The native `<audio controls>` output element **stays visible** (accessibility + fallback); the output waveform renders above it. All 10 existing audio Playwright specs must pass unchanged.
- Working branch: `audio-waveform` (off `tool-loop/2026-07-02`). Commit after every task.
- Build/tooling gotchas: `source $HOME/.cargo/env` first; `cargo run --manifest-path tools/generator/Cargo.toml -- .` runs from repo root; Playwright runs from `tests/` via `xvfb-run npx playwright test <spec>`.

---

### Task 1: Generator — parse `waveform` meta and bake `cfg.waveform`; copy `tool-audio.js`

**Files:**
- Modify: `tools/generator/src/meta.rs` (struct + client_config + tests)
- Modify: `tools/generator/src/main.rs:84` (asset copy list)

**Interfaces:**
- Consumes: nothing new.
- Produces: `window.GIZZA_TOOL.waveform` = `null` (absent → player enabled, unbound) | `false` (opt-out) | `{"start": "<field>", "end": "<field>"}` (bound). Tool pages with `runtime = "ffmpeg"` get `tool-audio.js` copied next to `tool.js`.

- [ ] **Step 1: Write the failing tests** — append to the `tests` module at the bottom of `tools/generator/src/meta.rs`:

```rust
    #[test]
    fn parses_waveform_binding_table() {
        let text = r#"
slug = "trim-audio"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "build_argv"
output_label = "o"
format = "audio"
runtime = "ffmpeg"

[waveform]
start = "start"
end = "end"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(
            m.waveform,
            Some(WaveformSpec::Binding { start: "start".into(), end: "end".into() })
        );
        let cfg = m.client_config();
        assert_eq!(cfg["waveform"]["start"], "start");
        assert_eq!(cfg["waveform"]["end"], "end");
    }

    #[test]
    fn parses_waveform_opt_out_and_default() {
        let base = r#"
slug = "x"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "run"
output_label = "o"
format = "audio"
"#;
        let off = format!("{base}waveform = false\n");
        let m = ToolMeta::from_toml(&off).unwrap();
        assert_eq!(m.waveform, Some(WaveformSpec::Enabled(false)));
        assert_eq!(m.client_config()["waveform"], serde_json::json!(false));

        let m = ToolMeta::from_toml(base).unwrap();
        assert_eq!(m.waveform, None);
        assert_eq!(m.client_config()["waveform"], serde_json::Value::Null);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `source $HOME/.cargo/env && cargo test --manifest-path tools/generator/Cargo.toml waveform`
Expected: FAIL — `WaveformSpec` not found / no field `waveform`.

- [ ] **Step 3: Implement.** In `tools/generator/src/meta.rs`, after the `Example` struct add:

```rust
/// Optional shared-waveform behavior. `waveform = false` disables the audio
/// waveform player on this page; `[waveform] start/end` names the two field
/// inputs the selection is two-way bound to (seconds). Absent = player
/// enabled, selection audition-only.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum WaveformSpec {
    Enabled(bool),
    Binding { start: String, end: String },
}
```

Add the field to `ToolMeta` (after `wide`):

```rust
    /// See [`WaveformSpec`]. Only audio-input ffmpeg tools consult this.
    #[serde(default)]
    pub waveform: Option<WaveformSpec>,
```

In `client_config()`, before the final `serde_json::json!({...})`, compute:

```rust
        let waveform = match &self.waveform {
            None => serde_json::Value::Null,
            Some(WaveformSpec::Enabled(b)) => serde_json::json!(b),
            Some(WaveformSpec::Binding { start, end }) => {
                serde_json::json!({ "start": start, "end": end })
            }
        };
```

and add `"waveform": waveform,` to the returned map (next to `"runtime"`).

In `tools/generator/src/main.rs`, inside the existing `runtime == "ffmpeg"` branch (directly after line 84's `tool-ffmpeg.js` copy):

```rust
            copy_file(&root.join("site/tool-audio.js"), &out.join("tool-audio.js"))?;
```

(`site/tool-audio.js` is created in Task 2; until then the generator would error — that is fine, Tasks 1+2 are committed together only after Task 2's syntax check, OR create an empty placeholder now: `echo '// created in Task 2' > site/tool-audio.js`. Do the placeholder so `cargo test` and a full generator run stay green.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `source $HOME/.cargo/env && cargo test --manifest-path tools/generator/Cargo.toml`
Expected: all generator tests PASS (including the two new ones).

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/meta.rs tools/generator/src/main.rs site/tool-audio.js
git commit -m "feat(generator): bake waveform meta (bind/opt-out) into GIZZA_TOOL; ship tool-audio.js to ffmpeg pages"
```

---

### Task 2: `site/tool-audio.js` module + `site/tool.css` styles

**Files:**
- Modify: `site/tool-audio.js` (replace the Task 1 placeholder with the real module)
- Modify: `site/tool.css` (append waveform styles)

**Interfaces:**
- Produces (consumed by Task 3/4/5 wiring):

```js
createWaveform(container, {
  interactive?: boolean,          // default true; false = output view (seek/play only)
  selection?: null | {
    getBounds(): {start: number, end: number|null},  // end null = "to end"
    onDrag(start: number, end: number): void,        // live during drag, no run
    onCommit(start: number, end: number): void,      // pointer release
  },
}) → {
  load(blob: Blob|File): Promise<boolean>,  // false = decode failed (container hidden)
  setSelection(start: number, end: number|null): void,
  clear(): void,                            // back to empty/hidden state (Reset)
  destroy(): void,
}
```

- [ ] **Step 1: Write the module.** Replace `site/tool-audio.js` with:

```js
// Shared interactive audio waveform for tool pages (audio-input ffmpeg tools
// and audio outputs). Dependency-free: WebAudio decode → canvas peaks, a
// hidden <audio> element for playback, Pointer Events for seek/select.
// Loaded lazily by tool.js; every failure path leaves the page's normal
// flow untouched (this component is an enhancement, never a gate).

const HEIGHT = 88;       // waveform lane height (CSS px, fixed — no layout jump)
const CLICK_PX = 4;      // movement under this = click (seek), not drag
const HANDLE_PX = 8;     // hit zone around a selection edge
const MIN_SEL_S = 0.05;  // selection edges can't cross closer than this

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

  let audioBuffer = null;
  let duration = 0;
  let objectUrl = null;
  let peaks = null; // Float32Array, [min,max] per canvas-css-px column
  let sel = null;   // {start, end} seconds, or null
  let playScope = null; // "all" | "selection" while playing
  let rafId = 0;

  const xOf = (t) => (duration ? (t / duration) * wave.clientWidth : 0);
  const tOf = (x) =>
    Math.max(0, Math.min(duration, (x / Math.max(1, wave.clientWidth)) * duration));

  function computePeaks(width) {
    const chans = [];
    for (let c = 0; c < audioBuffer.numberOfChannels; c++) {
      chans.push(audioBuffer.getChannelData(c));
    }
    const per = audioBuffer.length / width;
    const out = new Float32Array(width * 2);
    for (let x = 0; x < width; x++) {
      let mn = 1, mx = -1;
      const s = Math.floor(x * per);
      const e = Math.min(audioBuffer.length, Math.ceil((x + 1) * per));
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
    if (!audioBuffer || wave.clientWidth === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(wave.clientWidth * dpr);
    canvas.height = Math.round(HEIGHT * dpr);
    peaks = computePeaks(wave.clientWidth);
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
    if (!audioBuffer || e.button !== 0) return;
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
    if (!wasDrag) {
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
      try {
        const raw = await blob.arrayBuffer();
        const Ctx = window.AudioContext || window.webkitAudioContext;
        const ctx = new Ctx();
        try {
          // decodeAudioData detaches its buffer — hand it a copy.
          audioBuffer = await ctx.decodeAudioData(raw.slice(0));
        } finally {
          ctx.close();
        }
      } catch (err) {
        audioBuffer = null;
        container.hidden = true;
        return false;
      }
      duration = audioBuffer.duration;
      objectUrl = URL.createObjectURL(blob);
      audioEl.src = objectUrl;
      sel = null;
      if (binding) {
        const b = binding.getBounds();
        const s = Math.max(0, Math.min(duration, b.start || 0));
        const e = b.end == null || !Number.isFinite(b.end)
          ? duration
          : Math.max(s + MIN_SEL_S, Math.min(duration, b.end));
        sel = { start: s, end: e };
      }
      container.hidden = false;
      resize();
      updateBar();
      return true;
    },
    setSelection(start, end) {
      if (!audioBuffer) return;
      const s = Math.max(0, Math.min(duration, Number(start) || 0));
      const e = end == null || end === "" || !Number.isFinite(Number(end))
        ? duration
        : Math.max(s + MIN_SEL_S, Math.min(duration, Number(end)));
      sel = { start: s, end: e };
      draw();
      updateBar();
    },
    clear() {
      audioEl.pause();
      revoke();
      audioBuffer = null;
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
```

- [ ] **Step 2: Append styles to `site/tool.css`:**

```css
/* ---- Shared audio waveform (tool-audio.js) ---- */
.tool-wf {
  margin: 0.75rem 0;
}
.tool-wf-wave {
  position: relative;
  height: 88px;
  background: #f1f5f9;
  border: 1px solid #e2e8f0;
  border-radius: 8px;
  overflow: hidden;
  cursor: crosshair;
  touch-action: none; /* pointer events own horizontal drags */
}
.tool-wf-wave:focus-visible {
  outline: 2px solid #2563eb;
  outline-offset: 1px;
}
.tool-wf-canvas {
  display: block;
  width: 100%;
  height: 88px;
}
.tool-wf-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.4rem;
  min-height: 28px;
}
.tool-wf-btn {
  font: inherit;
  font-size: 0.82rem;
  padding: 0.2rem 0.7rem;
  border: 1px solid #cbd5e1;
  border-radius: 6px;
  background: #fff;
  cursor: pointer;
}
.tool-wf-btn:hover {
  background: #f8fafc;
}
.tool-wf-time {
  font-size: 0.78rem;
  font-variant-numeric: tabular-nums;
  color: #475569;
  margin-left: auto;
}
```

- [ ] **Step 3: Syntax-check the module**

Run: `node --check site/tool-audio.js && echo OK`
Expected: `OK` (behavioral verification lands with the Playwright specs in Tasks 3–5).

- [ ] **Step 4: Commit**

```bash
git add site/tool-audio.js site/tool.css
git commit -m "feat(site): shared tool-audio.js waveform component + styles"
```

---

### Task 3: Wire the input waveform in `site/tool.js` + Playwright spec (unbound tool)

**Files:**
- Modify: `site/tool.js` (ffmpeg branch, after `const fieldInputs = ...`, before `async function run()`; plus one insert inside the waveform block after `run` is defined — see code)
- Create: `tests/tool-page-audio-waveform.spec.ts`

**Interfaces:**
- Consumes: `createWaveform` (Task 2), `cfg.waveform` + baked `accept` (Task 1).
- Produces: `inputWf`/`outputWf` variables and a `wireWaveforms(run)` helper used by Task 4/5 (exact names below).

- [ ] **Step 1: Write the failing spec** — create `tests/tool-page-audio-waveform.spec.ts`:

```ts
import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// Shared waveform component (site/tool-audio.js): every audio-input ffmpeg
// tool page renders an interactive waveform after upload. audio-convert is
// the UNBOUND case: selection is audition-only and must never write fields.
// NOTE: two .tool-wf containers exist per page (input + output) — always
// disambiguate with .first()/.nth(1) to satisfy Playwright strict mode.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 3.03 s tone

test('audio-convert page renders a non-blank waveform after upload', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('.tool-wf').first()).toBeHidden();
  await page.setInputFiles('#in-audio', FIXTURE);
  await expect(page.locator('.tool-wf-wave').first()).toBeVisible({ timeout: 15_000 });
  // Canvas must actually contain drawn waveform pixels, not be blank.
  const drawn = await page.evaluate(() => {
    const c = document.querySelector('.tool-wf-canvas') as HTMLCanvasElement;
    const g = c.getContext('2d')!;
    const d = g.getImageData(0, 0, c.width, c.height).data;
    let painted = 0;
    for (let i = 3; i < d.length; i += 4) if (d[i] > 0) painted++;
    return painted;
  });
  expect(drawn).toBeGreaterThan(500);
});

test('audio-convert waveform plays and dragging writes no fields', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const wave = page.locator('.tool-wf-wave').first();
  await expect(wave).toBeVisible({ timeout: 15_000 });
  const bitrateBefore = await page.locator('#in-bitrate').inputValue();

  // Drag an audition selection across the middle of the waveform.
  const box = (await wave.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.25, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, y, { steps: 8 });
  await page.mouse.up();
  await expect(page.locator('.tool-wf-playsel').first()).toBeVisible();
  // Unbound tool: no field values changed by the drag.
  expect(await page.locator('#in-bitrate').inputValue()).toBe(bitrateBefore);

  // Play advances the underlying audio clock.
  await page.locator('.tool-wf-play').first().click();
  await page.waitForTimeout(400);
  const t = await page.evaluate(
    () => (document.querySelector('.tool-wf-time') as HTMLElement).textContent
  );
  expect(t).not.toContain('0:00.0 /');
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts`
Expected: FAIL — `.tool-wf-wave` never appears (tool.js doesn't create it yet).

- [ ] **Step 3: Wire tool.js.** In `site/tool.js`, inside the `cfg.runtime === "ffmpeg"` block, directly after `const fieldInputs = cfg.inputs.filter((i) => i.source === "field");` insert:

```js
    // ---- Shared audio waveform (site/tool-audio.js) ----------------------
    // Auto-enabled for audio-input tools (accept="audio/*") unless meta says
    // `waveform = false`. Declarative binding (cfg.waveform = {start,end})
    // two-way syncs the selection with those fields; commit runs ffmpeg once.
    // Enhancement only: every failure path leaves the normal flow working.
    let inputWf = null;
    let outputWf = null;
    const wfBinding =
      cfg.waveform && typeof cfg.waveform === "object" ? cfg.waveform : null;
    const wantWaveform =
      cfg.waveform !== false &&
      fileInput &&
      ((fileMeta && fileMeta.accept) || "").startsWith("audio/");

    async function wireWaveforms() {
      if (!wantWaveform) return;
      try {
        const { createWaveform } = await import("./tool-audio.js");
        const startEl = wfBinding ? document.getElementById("in-" + wfBinding.start) : null;
        const endEl = wfBinding ? document.getElementById("in-" + wfBinding.end) : null;
        const selection =
          startEl && endEl
            ? {
                getBounds: () => ({
                  start: parseFloat(startEl.value) || 0,
                  end: endEl.value === "" ? null : parseFloat(endEl.value),
                }),
                onDrag: (s, e) => {
                  // live field mirror — programmatic writes fire no events
                  startEl.value = s.toFixed(1);
                  endEl.value = e.toFixed(1);
                },
                onCommit: (s, e) => {
                  startEl.value = s.toFixed(1);
                  endEl.value = e.toFixed(1);
                  // exactly one change event → exactly one ffmpeg run
                  endEl.dispatchEvent(new Event("change"));
                },
              }
            : null;
        const inWrap = document.createElement("div");
        inWrap.hidden = true;
        fileInput.after(inWrap);
        inputWf = createWaveform(inWrap, { selection });
        if (selection) {
          for (const el of [startEl, endEl]) {
            el.addEventListener("input", () => {
              const b = selection.getBounds();
              inputWf.setSelection(b.start, b.end);
            });
          }
        }
        const outWrap = document.createElement("div");
        outWrap.hidden = true;
        media.before(outWrap);
        outputWf = createWaveform(outWrap, { interactive: false });

        fileInput.addEventListener("change", () => {
          const f = fileInput.files && fileInput.files[0];
          if (f) inputWf.load(f);
          else inputWf.clear();
        });
        const reset = document.getElementById("tool-reset");
        if (reset) {
          reset.addEventListener("click", () => {
            inputWf.clear();
            outputWf.clear();
          });
        }
      } catch (e) {
        inputWf = null;
        outputWf = null; // component failed to load — tool works as before
      }
    }
```

Then, immediately after the existing `wireWidgetChrome(run);` line in the same ffmpeg block, add:

```js
    await wireWaveforms();
```

(Placing it before the `custom.setup` early-return keeps custom-takeover tools untouched only if they return false; image-crop and friends are image tools, `wantWaveform` is false there.)

Finally, the `?url=` deep-link path calls `run()` directly and would skip the waveform's
file-change listener. Change the existing block at the bottom of the ffmpeg branch:

```js
    if (qpUrl && fileInput) {
      loadUrlIntoFile(qpUrl).then((ok) => {
        if (ok) run();
      });
    }
```

to dispatch the change event instead (one event → both the existing `run` listener and
the waveform load listener fire):

```js
    if (qpUrl && fileInput) {
      loadUrlIntoFile(qpUrl).then((ok) => {
        if (ok) fileInput.dispatchEvent(new Event("change"));
      });
    }
```

- [ ] **Step 4: Regenerate pages and run the spec**

```bash
source $HOME/.cargo/env
cargo run --manifest-path tools/generator/Cargo.toml -- . 2>&1 | tail -1
cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts
```
Expected: both tests PASS.

- [ ] **Step 5: Run one existing audio spec as an early regression probe**

Run: `cd tests && xvfb-run npx playwright test tool-page-audio-convert.spec.ts`
Expected: PASS unchanged.

- [ ] **Step 6: Commit**

```bash
git add site/tool.js tests/tool-page-audio-waveform.spec.ts
git commit -m "feat(site): auto-wire input waveform on audio tool pages (audition-only by default)"
```

---

### Task 4: trim-audio selection binding (meta + spec)

**Files:**
- Modify: `blocks/trim-audio/page/meta.toml` (add `[waveform]` table at the END of the file — TOML: a table header captures everything after it, so it must come after all `[[input]]` entries… place it after the last `[[input]]`)
- Modify: `tests/tool-page-audio-waveform.spec.ts` (add bound-tool tests)

**Interfaces:**
- Consumes: `cfg.waveform` binding (Task 1), `wireWaveforms` (Task 3).
- Produces: the user-visible two-way binding on /tools/trim-audio/.

- [ ] **Step 1: Add the failing tests** — append to `tests/tool-page-audio-waveform.spec.ts`:

```ts
async function decodeDurationOfDataUrl(page: Page, src: string): Promise<number> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src);
}

test('trim-audio drag-selection writes start/end and trims to the selection', async ({ page }) => {
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const wave = page.locator('.tool-wf-wave').first();
  await expect(wave).toBeVisible({ timeout: 15_000 });

  // Drag 25% → 50% of a 3.03 s tone ⇒ start ≈ 0.76, end ≈ 1.52.
  const box = (await wave.boundingBox())!;
  const y = box.y + box.height / 2;
  await page.mouse.move(box.x + box.width * 0.25, y);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.5, y, { steps: 8 });
  await page.mouse.up();

  const start = parseFloat(await page.locator('#in-start').inputValue());
  const end = parseFloat(await page.locator('#in-end').inputValue());
  expect(start).toBeGreaterThan(0.55);
  expect(start).toBeLessThan(0.95);
  expect(end).toBeGreaterThan(1.3);
  expect(end).toBeLessThan(1.75);

  // The commit fired one run; the trimmed output matches the selection length.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const dur = await decodeDurationOfDataUrl(page, src!);
  expect(Math.abs(dur - (end - start))).toBeLessThan(0.2);
});

test('trim-audio typing start/end moves the selection highlight', async ({ page }) => {
  await page.goto('/tools/trim-audio/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  await expect(page.locator('.tool-wf-wave').first()).toBeVisible({ timeout: 15_000 });
  await page.locator('#in-start').fill('1');
  await page.locator('#in-end').fill('2');
  // The bar's selection readout mirrors the typed values (0:01.0–0:02.0 (1.0s)).
  await expect(page.locator('.tool-wf-time').first()).toContainText('0:01.0–0:02.0');
});
```

- [ ] **Step 2: Run to verify the new tests fail**

Run: `cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts`
Expected: the two trim-audio tests FAIL (no binding: fields don't change / readout shows no selection). Task 3's two tests still PASS.

- [ ] **Step 3: Declare the binding.** Append to `blocks/trim-audio/page/meta.toml` (very end of file):

```toml
[waveform]
start = "start"
end   = "end"
```

- [ ] **Step 4: Regenerate and run**

```bash
source $HOME/.cargo/env
cargo run --manifest-path tools/generator/Cargo.toml -- . 2>&1 | tail -1
cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts tool-page-trim-audio.spec.ts
```
Expected: all waveform tests PASS **and** the existing trim-audio spec PASSES unchanged (binding reads prefilled fields, never clobbers them).

- [ ] **Step 5: Commit**

```bash
git add blocks/trim-audio/page/meta.toml tests/tool-page-audio-waveform.spec.ts
git commit -m "feat(trim-audio): two-way waveform selection bound to start/end"
```

---

### Task 5: Output waveform after each run

**Files:**
- Modify: `site/tool.js` (inside `async function run()`, the `r.ok` branch)
- Modify: `tests/tool-page-audio-waveform.spec.ts` (output assertions)

**Interfaces:**
- Consumes: `outputWf` (Task 3). The native `#tool-output-media` stays visible (existing specs assert it).

- [ ] **Step 1: Add the failing test** — append to the spec:

```ts
test('audio-convert result renders an output waveform above the native player', async ({ page }) => {
  await page.goto('/tools/audio-convert/');
  await page.waitForSelector('#in-audio');
  await page.selectOption('#in-format', 'wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  // Two waveforms now: input + output. Output one is the second .tool-wf.
  const waves = page.locator('.tool-wf-wave');
  await expect(waves).toHaveCount(2);
  await expect(waves.nth(1)).toBeVisible();
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts -g "output waveform"`
Expected: FAIL — only one `.tool-wf-wave` exists.

- [ ] **Step 3: Implement.** In `site/tool.js`'s `run()`, the success branch currently reads:

```js
      if (r.ok) {
        out.textContent = "";
        media.src = r.dataUrl;
        media.hidden = false;
        dl.href = r.dataUrl;
        dl.download = r.outName;
        dl.hidden = false;
      } else {
```

Replace with:

```js
      if (r.ok) {
        out.textContent = "";
        media.src = r.dataUrl;
        media.hidden = false;
        dl.href = r.dataUrl;
        dl.download = r.outName;
        dl.hidden = false;
        // Visual result: decode the output into the read-only waveform. The
        // native <audio controls> stays visible (accessible transport +
        // decode-failure fallback); the waveform adds the before/after view.
        if (outputWf && String(r.dataUrl).startsWith("data:audio/")) {
          try {
            const blob = await (await fetch(r.dataUrl)).blob();
            await outputWf.load(blob);
          } catch (e) {
            outputWf.clear(); // fallback: native player alone, as today
          }
        }
      } else {
```

Also, two lines earlier in `run()` where the pre-run state hides the media (`media.hidden = true; dl.hidden = true;`), add:

```js
      if (outputWf) outputWf.clear();
```

- [ ] **Step 4: Run the full new spec**

Run: `cd tests && xvfb-run npx playwright test tool-page-audio-waveform.spec.ts`
Expected: all 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add site/tool.js tests/tool-page-audio-waveform.spec.ts
git commit -m "feat(site): render tool results as a read-only output waveform"
```

---

### Task 6: Full regression, UX screenshot, PR

**Files:**
- No new code. Regenerated `pkg/` pages (gitignored), verification only.

- [ ] **Step 1: Generator unit tests + hygiene**

```bash
source $HOME/.cargo/env
cargo test --manifest-path tools/generator/Cargo.toml
python3 scripts/check-tool-hygiene.py trim-audio
```
Expected: PASS / exit 0.

- [ ] **Step 2: Full audio-family Playwright regression**

```bash
cd tests && xvfb-run npx playwright test \
  tool-page-audio-waveform.spec.ts tool-page-trim-audio.spec.ts \
  tool-page-audio-convert.spec.ts tool-page-audio-normalize.spec.ts \
  tool-page-audio-silence-remove.spec.ts tool-page-audio-to-mono.spec.ts \
  tool-page-audio-volume-adjust.spec.ts tool-page-audio-compress.spec.ts \
  tool-page-audio-eq.spec.ts tool-page-audio-fade.spec.ts \
  tool-page-audio-loop.spec.ts tool-page-widget-chrome.spec.ts
```
Expected: ALL PASS. Any existing-spec failure is a regression — fix the wiring, not the spec.

- [ ] **Step 3: UX screenshot review** — serve `pkg/` (`cd pkg && python3 -m http.server 8931 &`), load `/tools/trim-audio/` via playwright-MCP, upload nothing (empty state must show no waveform box), then screenshot `/tools/trim-audio/` and `/tools/audio-convert/` after a fixture upload; verify: waveform matches site styling, no layout jump, transport row readable. Kill server (`fuser -k 8931/tcp`), delete screenshots.

- [ ] **Step 4: Push + PR**

```bash
git push -u origin audio-waveform
gh pr create --base tool-loop/2026-07-02 --head audio-waveform \
  --title "feat(site): interactive audio waveform on all audio tool pages" \
  --body "Shared dependency-free waveform (see/scrub/audition + select) auto-enabled for audio tools; trim-audio selection two-way bound to start/end; results render as waveforms. Spec: docs/superpowers/specs/2026-07-03-audio-waveform-design.md"
```
(Base is `tool-loop/2026-07-02` while PR #160 is open; retarget to `main` after it merges.)
