// Generic standalone-tool driver. Reads window.GIZZA_TOOL (baked by the page
// generator), loads the tool's wasm-bindgen module, wires inputs to the
// exported function, and renders the result. Shared by every tool page (/tools/<slug>/).

import { queryPrefill } from "./query-prefill.js";

const cfg = window.GIZZA_TOOL;
const out = document.getElementById(cfg.output.elementId);

function showResult(value) {
  out.classList.remove("error");
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
}

function showError(message) {
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

// Collect call args in declared order. "field" → input value; "clock" → now (s).
function gatherArgs() {
  return cfg.inputs.map((inp) => {
    if (inp.source === "clock") return Math.floor(Date.now() / 1000);
    const el = document.getElementById(inp.elementId);
    return el ? el.value : "";
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
  if (cfg.runtime === "ffmpeg") {
    const { runFfmpeg } = await import("./tool-ffmpeg.js");
    const { ffmpegExec } = await import("./ffmpeg.js");
    const media = document.getElementById("tool-output-media");
    const dl = document.getElementById("tool-output-download");
    if (!media || !dl) {
      // ffmpeg runtime requires a media output (format "image"/"video"); a
      // misconfigured tool (e.g. runtime=ffmpeg + format=text) has no place to
      // render the result. Fail loudly instead of throwing on a null element.
      showError("tool misconfigured: ffmpeg runtime needs an image/video output");
      return;
    }
    const fileMeta = cfg.inputs.find((i) => i.source === "file");
    const fileInput = fileMeta ? document.getElementById("in-" + fileMeta.name) : null;
    const fieldInputs = cfg.inputs.filter((i) => i.source === "field");

    async function run() {
      const file = fileInput && fileInput.files && fileInput.files[0];
      if (!file) return;
      out.textContent = "Processing…";
      out.classList.remove("error");
      media.hidden = true;
      dl.hidden = true;
      // Coerce numeric-looking field values to Number so wasm-bindgen f64 params
      // marshal correctly; leave non-numeric (e.g. "contain") and empty strings
      // as strings — the WASM function handles empty via its own defaults.
      const fieldArgs = fieldInputs.map((i) => {
        const el = document.getElementById(i.elementId);
        const v = el ? el.value : "";
        return v !== "" && !isNaN(Number(v)) ? Number(v) : v;
      });
      const r = await runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs);
      if (r.ok) {
        out.textContent = "";
        media.src = r.dataUrl;
        media.hidden = false;
        dl.href = r.dataUrl;
        dl.download = r.outName;
        dl.hidden = false;
      } else {
        showError(r.error);
      }
    }

    // Deep-link: prefill scalar fields; if ?url= is present, fetch the remote
    // media into the file input and auto-run. Param names == input names.
    const { fields: qpFields, url: qpUrl } = queryPrefill(cfg.inputs, location.search);
    for (const f of qpFields) {
      const el = document.getElementById(f.elementId);
      if (el) el.value = f.value;
    }
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

    if (fileInput) fileInput.addEventListener("change", run);
    for (const i of fieldInputs) {
      const el = document.getElementById(i.elementId);
      if (el) el.addEventListener("input", run);
    }
    if (qpUrl && fileInput) {
      loadUrlIntoFile(qpUrl).then((ok) => {
        if (ok) run();
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
    const el = document.getElementById(f.elementId);
    if (el) el.value = f.value;
  }

  // Wire field inputs to live recompute.
  for (const inp of cfg.inputs) {
    if (inp.source === "field") {
      const el = document.getElementById(inp.elementId);
      if (el) el.addEventListener("input", compute);
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
