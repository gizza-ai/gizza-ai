// Generic standalone-tool driver. Reads window.GIZZA_TOOL (baked by the page
// generator), loads the tool's wasm-bindgen module, wires inputs to the
// exported function, and renders the result. Shared by every tool page (/tools/<slug>/).

import { queryPrefill } from "./query-prefill.js";

const cfg = window.GIZZA_TOOL;
const out = document.getElementById(cfg.output.elementId);

// Per-tool escape hatch (cfg.custom → ./custom.js, which the generator copies
// from blocks/<slug>/page/custom.js). Optional exports:
//   setup(ctx)                → one-time setup; return true to TAKE OVER wiring
//   renderResult(value, ctx)  → return true if the result was rendered
//   renderError(message, ctx) → return true if the error was rendered
// Prefer the declarative meta.toml controls (kind incl. slider/options/default/
// [[example]]/wide); custom.js is only for layouts/renderers those can't
// express. Do NOT add per-tool slug branches to this file.
let custom = {};
let customCtx = null;

function showResult(value) {
  out.classList.remove("error");
  if (custom.renderResult && custom.renderResult(value, customCtx)) {
    return;
  }
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
}

function showError(message) {
  // Layout stability: never resize the widget on errors/keystrokes — nothing
  // may jump under the user's cursor. Wide layouts are the tool-widget--wide
  // class (meta.toml `wide = true`), not a JS width override.
  if (custom.renderError && custom.renderError(message, customCtx)) {
    return;
  }
  out.classList.add("error");
  out.textContent = message;
}


function formatNumber(v) {
  if (!Number.isFinite(v)) return String(v);
  // Trim float noise without forcing decimals on integers — but only when the
  // *1e12 scaling stays finite. For very large magnitudes (|v| > ~1.8e296) the
  // scaling would overflow to Infinity and misreport a valid finite result, so
  // fall back to the unrounded value there.
  const scaled = Math.round(v * 1e12) / 1e12;
  return Number.isFinite(scaled) ? String(scaled) : String(v);
}

// Read a field element as the string the wasm export expects. A checkbox
// yields "true"/"false" (the wasm side parses booleans from strings); a
// <select>/<input>/<textarea> yields its value.
function readField(el) {
  if (!el) return "";
  if (el.type === "checkbox") return el.checked ? "true" : "false";
  return el.value;
}

// Apply a deep-link prefill value to a field element (checkbox vs value-bearing).
function applyField(el, value) {
  if (!el) return;
  if (el.type === "checkbox") {
    el.checked = ["true", "1", "yes", "on"].includes(String(value).toLowerCase());
  } else {
    el.value = value;
  }
}

// ---- Declarative widget behaviors (defaults / example chips / reset / copy) ----
// Driven by window.GIZZA_TOOL, which the generator bakes from page/meta.toml —
// every page gets these with ZERO per-tool JS. Do not add per-tool slug
// branches here; extend the meta/generator instead (or, as a last resort, the
// tool's page/custom.js) — workspace fix-at-root-cause rule.

// Resolve a meta `default` spec: "today"/"now" → the user's local date(-time),
// "local-timezone" → their IANA zone, anything else literal.
function resolveDefault(spec) {
  const pad = (n) => String(n).padStart(2, "0");
  const now = new Date();
  if (spec === "today") {
    return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  }
  if (spec === "now") {
    return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}:${pad(now.getMinutes())}`;
  }
  if (spec === "local-timezone") {
    try {
      return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    } catch (e) {
      return "UTC";
    }
  }
  return spec;
}

// Apply meta-declared defaults to empty fields. URL-prefilled or already-filled
// fields are left alone unless `force` (the Reset path).
function applyMetaDefaults(force = false) {
  const params = new URLSearchParams(location.search);
  for (const inp of cfg.inputs) {
    if (inp.source !== "field" || !inp.default) continue;
    const el = document.getElementById(inp.elementId);
    if (!el) continue;
    if (!force && (params.has(inp.name) || el.value)) continue;
    applyField(el, resolveDefault(inp.default));
  }
}

// Wire the example chips + Reset + Copy-result chrome the template renders.
// `rerun` recomputes after programmatic input changes (compute for pure tools;
// for ffmpeg tools it is the run fn, a no-op until a file is chosen).
function wireWidgetChrome(rerun) {
  for (const btn of document.querySelectorAll(".tool-example-chip")) {
    btn.addEventListener("click", () => {
      const ex = (cfg.examples || [])[Number(btn.dataset.example)];
      if (!ex) return;
      for (const [name, value] of Object.entries(ex.params || {})) {
        const inp = cfg.inputs.find((i) => i.name === name);
        if (inp) applyField(document.getElementById(inp.elementId), value);
      }
      refreshTagLists();
      refreshSliders();
      rerun();
    });
  }

  const reset = document.getElementById("tool-reset");
  if (reset) {
    reset.addEventListener("click", () => {
      for (const inp of cfg.inputs) {
        const el = document.getElementById(inp.elementId);
        if (!el) continue;
        if (inp.source === "file") {
          el.value = "";
          continue;
        }
        if (inp.source !== "field") continue;
        // defaultValue/defaultChecked restore the server-rendered state (the
        // select's default option, the checkbox default, a number's default).
        if (el.type === "checkbox") {
          el.checked = el.defaultChecked;
        } else if (el.tagName === "SELECT") {
          for (const o of el.options) o.selected = o.defaultSelected;
        } else {
          el.value = el.defaultValue;
        }
      }
      applyMetaDefaults(true);
      refreshTagLists();
      refreshSliders();
      const media = document.getElementById("tool-output-media");
      const dl = document.getElementById("tool-output-download");
      if (media) media.hidden = true;
      if (dl) dl.hidden = true;
      // A previous run's error/result must not survive Reset (rerun()
      // early-returns without recomputing when e.g. no file is selected).
      out.classList.remove("error");
      out.textContent = "";
      history.replaceState({}, document.title, location.pathname);
      rerun();
    });
  }

  const copy = document.getElementById("tool-copy-output");
  if (copy) {
    copy.addEventListener("click", async () => {
      const text = (out.textContent || "").trim();
      if (!text) return;
      try {
        await navigator.clipboard.writeText(text);
        copy.classList.add("copied");
        const prev = copy.textContent;
        copy.textContent = "Copied!";
        setTimeout(() => {
          copy.classList.remove("copied");
          copy.textContent = prev;
        }, 1500);
      } catch (e) {
        // Clipboard unavailable (e.g. non-secure context) — button is best-effort.
      }
    });
  }
}

// ---- Generic slider mirror (meta kind="slider") ----
// A range input (id "in-<name>-slider", data-for="in-<name>") mirrors the
// canonical number box two-way. Dragging updates the number live WITHOUT
// firing events; releasing dispatches ONE change on the number input — one
// run per drag, the same commit discipline as the waveform selection. Pure
// (non-ffmpeg) tools also recompute live during the drag, which is cheap.

// Where the thumb rests when its number box is empty: the rendered value
// attribute (the schema default) or the numeric midpoint (the range
// element's own no-value default).
function sliderRestValue(slider) {
  if (slider.defaultValue !== "") return slider.defaultValue;
  const min = Number(slider.min);
  const max = Number(slider.max);
  return String(min + (max - min) / 2);
}

// Re-sync every slider from its number box (after programmatic value writes:
// meta defaults, example chips, reset, deep-links).
function refreshSliders() {
  for (const s of document.querySelectorAll(".tool-slider")) {
    const el = document.getElementById(s.dataset.for);
    if (!el) continue;
    s.value = el.value !== "" ? el.value : sliderRestValue(s);
  }
}

function wireSliders() {
  for (const s of document.querySelectorAll(".tool-slider")) {
    const el = document.getElementById(s.dataset.for);
    if (!el) continue;
    s.addEventListener("input", () => {
      el.value = s.value; // live mirror — programmatic writes fire no events
      if (cfg.runtime !== "ffmpeg") el.dispatchEvent(new Event("input"));
    });
    s.addEventListener("change", () => {
      el.value = s.value;
      el.dispatchEvent(new Event("change")); // exactly one run per drag-release
    });
    el.addEventListener("input", () => {
      if (el.value !== "") s.value = el.value;
    });
  }
  refreshSliders();
}

// ---- Generic tag-list widget (meta kind="tag-list") ----
// Pills + a search box, backed by the HIDDEN comma-joined input the wasm
// actually reads (so gatherArgs/deep-links/CLI parity need no special-casing).

function tagValues(hidden) {
  return hidden.value.split(",").map((s) => s.trim()).filter(Boolean);
}

function renderTagList(container) {
  const hidden = document.getElementById(container.dataset.input);
  const list = container.querySelector(".tool-tags-list");
  if (!hidden || !list) return;
  list.innerHTML = "";
  const values = tagValues(hidden);
  if (!values.length) {
    const empty = document.createElement("span");
    empty.className = "tool-tags-empty";
    empty.textContent = "Nothing added yet.";
    list.appendChild(empty);
    return;
  }
  for (const v of values) {
    const pill = document.createElement("span");
    pill.className = "tool-tag-pill";
    pill.append(v + " ");
    const del = document.createElement("button");
    del.type = "button";
    del.className = "tool-tag-del";
    del.setAttribute("aria-label", `Remove ${v}`);
    del.innerHTML = "&times;";
    del.addEventListener("click", () => {
      hidden.value = tagValues(hidden).filter((t) => t !== v).join(", ");
      renderTagList(container);
      hidden.dispatchEvent(new Event("change"));
    });
    pill.appendChild(del);
    list.appendChild(pill);
  }
}

// Re-render every tag list from its hidden input (after programmatic value
// changes: meta defaults, example chips, reset, deep-links).
function refreshTagLists() {
  for (const c of document.querySelectorAll(".tool-tags")) renderTagList(c);
}

function wireTagLists() {
  for (const container of document.querySelectorAll(".tool-tags")) {
    const hidden = document.getElementById(container.dataset.input);
    const search = container.querySelector(".tool-tags-search");
    const addBtn = container.querySelector(".tool-tags-add-btn");
    if (!hidden || !search) continue;
    const listId = search.getAttribute("list");
    const allowed = listId
      ? Array.from(document.querySelectorAll(`#${listId} option`)).map((o) => o.value)
      : null;
    const add = () => {
      let v = search.value.trim();
      if (!v) return;
      if (allowed) {
        // Vocabulary-restricted: only known values, canonical casing.
        const match = allowed.find((a) => a.toLowerCase() === v.toLowerCase());
        if (!match) return;
        v = match;
      }
      const values = tagValues(hidden);
      if (!values.includes(v)) {
        values.push(v);
        hidden.value = values.join(", ");
        renderTagList(container);
        hidden.dispatchEvent(new Event("change"));
      }
      search.value = "";
    };
    if (addBtn) addBtn.addEventListener("click", add);
    search.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        add();
      }
    });
    // Picking a datalist entry fires `input` with the complete value — auto-add.
    search.addEventListener("input", () => {
      const v = search.value.trim();
      if (allowed && allowed.some((a) => a.toLowerCase() === v.toLowerCase())) add();
    });
    renderTagList(container);
  }
}

// Collect call args in declared order. "field" → input value; "clock" → now (s).
function gatherArgs() {
  return cfg.inputs.map((inp) => {
    if (inp.source === "clock") return Math.floor(Date.now() / 1000);
    return readField(document.getElementById(inp.elementId));
  });
}

async function main() {
  let mod;
  try {
    mod = await import(cfg.module);
    await mod.default(); // wasm-pack --target web init
  } catch (e) {
    showError("Failed to load tool.");
    return;
  }
  if (cfg.custom) {
    try {
      custom = await import("./custom.js");
    } catch (e) {
      custom = {}; // a broken custom module must not take the whole page down
    }
  }
  customCtx = {
    cfg,
    mod,
    out,
    helpers: {
      applyField,
      readField,
      gatherArgs,
      showResult,
      showError,
      resolveDefault,
      applyMetaDefaults,
      refreshTagLists,
      formatNumber,
    },
  };
  if (cfg.runtime === "ffmpeg") {
    const { runFfmpeg } = await import("./tool-ffmpeg.js");
    const { ffmpegExec } = await import("./ffmpeg.js");
    const media = document.getElementById("tool-output-media");
    const dl = document.getElementById("tool-output-download");
    if (!media || !dl) {
      // ffmpeg runtime requires a media output (format "image"/"video"/"audio");
      // a misconfigured tool (e.g. runtime=ffmpeg + format=text) has no place to
      // render the result. Fail loudly instead of throwing on a null element.
      showError("tool misconfigured: ffmpeg runtime needs an image/video/audio output");
      return;
    }
    const fileMeta = cfg.inputs.find((i) => i.source === "file");
    const fileInput = fileMeta ? document.getElementById("in-" + fileMeta.name) : null;
    const fieldInputs = cfg.inputs.filter((i) => i.source === "field");

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
        // Round a dragged [s, e] outward to 0.1 s field granularity, keeping
        // end at least one step above start.
        const roundBounds = (s, e) => {
          const fs = Math.max(0, Math.floor(s * 10) / 10);
          const fe = Math.max(Math.ceil(e * 10) / 10, fs + 0.1);
          return [fs.toFixed(1), fe.toFixed(1)];
        };
        const selection =
          startEl && endEl
            ? {
                // Raw field values; interpretation (0/empty/invalid end =
                // unbounded) is normalizeSel's job in tool-audio.js.
                getBounds: () => ({
                  start: parseFloat(startEl.value) || 0,
                  end: endEl.value === "" ? null : parseFloat(endEl.value),
                }),
                // Field mirror rounds OUTWARD to the fields' 0.1 s granularity
                // and keeps end a step above start, so a micro-drag can never
                // commit start==end (or 0/0, the whole-file sentinel).
                onDrag: (s, e) => {
                  // live mirror — programmatic writes fire no events
                  const [fs, fe] = roundBounds(s, e);
                  startEl.value = fs;
                  endEl.value = fe;
                },
                onCommit: (s, e) => {
                  const [fs, fe] = roundBounds(s, e);
                  startEl.value = fs;
                  endEl.value = fe;
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

    // Overlapping runs race: ffmpeg run times vary, so a stale slow run can
    // resolve after a newer one and overwrite its media.src and output
    // waveform — or repaint output that Reset just cleared. Every run() call
    // takes a ticket (including the no-file early return, so Reset's rerun
    // invalidates an in-flight run); only the newest ticket may touch the DOM.
    let runSeq = 0;

    async function run() {
      const seq = ++runSeq;
      const file = fileInput && fileInput.files && fileInput.files[0];
      if (!file) return;
      out.textContent = "Processing…";
      out.classList.remove("error");
      media.hidden = true;
      dl.hidden = true;
      if (outputWf) outputWf.clear();
      // Coerce numeric-looking field values to Number so wasm-bindgen f64 params
      // marshal correctly; leave non-numeric (e.g. "contain") and empty strings
      // as strings — the WASM function handles empty via its own defaults.
      const fieldArgs = fieldInputs.map((i) => {
        const el = document.getElementById(i.elementId);
        const v = el ? el.value : "";
        return v !== "" && !isNaN(Number(v)) ? Number(v) : v;
      });
      const r = await runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs);
      if (seq !== runSeq) return; // superseded while ffmpeg ran — drop the result
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
            if (seq === runSeq) await outputWf.load(blob);
          } catch (e) {
            if (seq === runSeq) outputWf.clear(); // fallback: native player alone, as today
          }
        }
      } else {
        showError(r.error);
      }
    }

    // Deep-link: prefill scalar fields; if ?url= is present, fetch the remote
    // media into the file input and auto-run. Param names == input names.
    const { fields: qpFields, url: qpUrl } = queryPrefill(cfg.inputs, location.search);
    for (const f of qpFields) {
      applyField(document.getElementById(f.elementId), f.value);
    }
    applyMetaDefaults();
    wireTagLists();
    wireSliders();
    wireWidgetChrome(run);
    if (custom.setup && custom.setup({ ...customCtx, run, fileInput, fieldInputs }) === true) {
      return; // custom module owns all wiring for this tool
    }
    await wireWaveforms();
    async function loadUrlIntoFile(url) {
      try {
        const resp = await fetch(url);
        if (!resp.ok) throw new Error("HTTP " + resp.status);
        const blob = await resp.blob();
        const name = (url.split("/").pop() || "input").split("?")[0] || "input";
        const dt = new DataTransfer();
        dt.items.add(new File([blob], name, { type: blob.type }));
        fileInput.files = dt.files;
        return true;
      } catch (e) {
        showError(
          "Couldn't fetch " + url + " — the host may block cross-origin access. " +
            "Download it and choose the file instead."
        );
        return false;
      }
    }

    if (fileInput) {
      fileInput.addEventListener("change", run);
    }
    for (const i of fieldInputs) {
      const el = document.getElementById(i.elementId);
      if (el) {
        el.addEventListener("input", run);
        el.addEventListener("change", run); // <select>/checkbox fire change, not input
      }
    }
    if (qpUrl && fileInput) {
      loadUrlIntoFile(qpUrl).then((ok) => {
        if (ok) fileInput.dispatchEvent(new Event("change"));
      });
    }
    return;
  }

  const fn = mod[cfg.export];

  function compute() {
    try {
      const result = fn(...gatherArgs());
      showResult(result);
    } catch (e) {
      const msg = typeof e === "string" ? e : e && e.message ? e.message : "error";
      // Don't shout at the user for an empty field.
      const hasField = cfg.inputs.some((i) => i.source === "field");
      const empty = hasField && gatherArgs().every((a) => a === "" || a == null);
      if (empty) {
        out.classList.remove("error");
        out.textContent = "";
      } else {
        showError(msg);
      }
    }
  }

  // Deep-link: prefill fields from the URL query, then the initial compute()
  // below auto-runs with those values. Param names == input names.
  for (const f of queryPrefill(cfg.inputs, location.search).fields) {
    applyField(document.getElementById(f.elementId), f.value);
  }
  applyMetaDefaults();
  wireTagLists();
  wireSliders();
  wireWidgetChrome(compute);
  if (custom.setup && custom.setup({ ...customCtx, compute }) === true) {
    return; // custom module owns all wiring for this tool
  }

  // Wire field inputs to live recompute.
  for (const inp of cfg.inputs) {
    if (inp.source === "field") {
      const el = document.getElementById(inp.elementId);
      if (el) {
        el.addEventListener("input", compute);
        el.addEventListener("change", compute); // <select>/checkbox fire change, not input
      }
    }
  }

  if (cfg.live) {
    compute();
    setInterval(compute, cfg.intervalMs || 1000);
  } else {
    compute(); // initial (e.g. prefilled / empty state)
  }
}

main();

