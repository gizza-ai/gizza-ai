// image-resize-to-filesize page module — target-file-size needs a SEARCH, not a
// single ffmpeg pass, so this tool fully owns its wiring (setup returns true).
// For each candidate quality it asks the shared wasm (`mod.build_attempt`) for
// the ffmpeg argv, runs it through ffmpegExec, and measures the output — the
// exact mirror of the chat/CLI block's core::search_quality binary search, with
// the SAME Q_MIN/Q_MAX bounds (keep these three in sync: core/src/lib.rs,
// src/lib.rs, here).
//
// Loaded by the shared tool.js via the generator's page/custom.js hook.

import { ffmpegExec } from "./ffmpeg.js";

// MUST match gizza_ai_image_resize_to_filesize_core::Q_MIN / Q_MAX.
const Q_MIN = 5;
const Q_MAX = 95;

export function setup(ctx) {
  const { mod, out, helpers, fileInput, fieldInputs } = ctx;
  const media = document.getElementById("tool-output-media");
  const dl = document.getElementById("tool-output-download");
  if (!mod || !fileInput || !media || !dl) return false; // fall back to shared flow

  const fieldEl = (name) => {
    const meta = fieldInputs.find((i) => i.name === name);
    return meta ? document.getElementById(meta.elementId) : null;
  };
  const targetKbEl = fieldEl("target_kb");
  const formatEl = fieldEl("format");
  const maxWidthEl = fieldEl("max_width");

  // Overlapping runs race (ffmpeg times vary); only the newest ticket may paint.
  let runSeq = 0;

  async function runSearch() {
    const seq = ++runSeq;
    const file = fileInput.files && fileInput.files[0];
    if (!file) return; // nothing to do until a file is chosen

    const targetKb = parseFloat(targetKbEl && targetKbEl.value);
    if (!isFinite(targetKb) || targetKb < 1) {
      // Don't shout before the user has typed a real target.
      out.classList.remove("error");
      out.textContent = "Enter a target size of at least 1 KB, then choose an image.";
      return;
    }
    const format = (formatEl && formatEl.value) || "jpg";
    const maxWidth = Math.max(0, Math.round(parseFloat(maxWidthEl && maxWidthEl.value) || 0));
    const targetBytes = Math.round(targetKb * 1024);

    out.classList.remove("error");
    out.textContent = "Searching for the best quality…";
    media.hidden = true;
    dl.hidden = true;

    const inName = inputNameFor(file.name);
    const bytesB64 = bytesToB64(new Uint8Array(await file.arrayBuffer()));
    if (seq !== runSeq) return;
    const inputsJson = JSON.stringify([{ name: inName, bytes_b64: bytesB64 }]);

    const cache = {}; // quality -> { b64, size, outName }
    async function probe(q) {
      if (cache[q]) return cache[q];
      let plan;
      try {
        plan = mod.build_attempt(format, q, maxWidth, inName);
      } catch (e) {
        throw new Error(typeof e === "string" ? e : (e && e.message) || "invalid arguments");
      }
      const resp = await ffmpegExec(JSON.stringify(plan.argv), inputsJson, plan.out_name);
      if (resp.exit_code !== 0 || !resp.output_b64) {
        const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
        throw new Error(snippet);
      }
      const r = { b64: resp.output_b64, size: b64ByteLen(resp.output_b64), outName: plan.out_name };
      cache[q] = r;
      return r;
    }

    let lo = Q_MIN;
    let hi = Q_MAX;
    let best = null; // highest quality that fits
    let smallest = null; // smallest output seen (fallback)
    try {
      while (lo <= hi) {
        const mid = lo + ((hi - lo) >> 1);
        const r = await probe(mid);
        if (seq !== runSeq) return; // superseded mid-search
        if (!smallest || r.size < smallest.size) smallest = { q: mid, ...r };
        if (r.size <= targetBytes) {
          best = { q: mid, ...r };
          lo = mid + 1; // spend the budget on higher quality
        } else if (mid === Q_MIN) {
          break;
        } else {
          hi = mid - 1;
        }
      }
    } catch (e) {
      if (seq === runSeq) {
        out.classList.add("error");
        out.textContent = e.message || "encoding failed";
      }
      return;
    }
    if (seq !== runSeq) return;

    const chosen = best || smallest;
    if (!chosen) {
      out.classList.add("error");
      out.textContent = "Encoding failed — the image format may be unsupported.";
      return;
    }
    const mime = format === "webp" ? "image/webp" : "image/jpeg";
    const dataUrl = `data:${mime};base64,${chosen.b64}`;
    media.src = dataUrl;
    media.hidden = false;
    dl.href = dataUrl;
    dl.download = chosen.outName;
    dl.hidden = false;
    const kb = (chosen.size / 1024).toFixed(1);
    const widthNote = maxWidth ? `, max width ${maxWidth}px` : "";
    out.classList.remove("error");
    out.textContent = best
      ? `Done — ${kb} KB at quality ${chosen.q} (target ${targetKb} KB${widthNote}).`
      : `Smallest reachable was ${kb} KB at quality ${chosen.q}, still over the ${targetKb} KB target — try a smaller Max width.`;
  }

  // Wire inputs. `change` only (not `input`) so we don't launch a whole search
  // on every keystroke; the file input change and each committed field change
  // re-run the search.
  fileInput.addEventListener("change", runSearch);
  for (const el of [targetKbEl, formatEl, maxWidthEl]) {
    if (el) el.addEventListener("change", runSearch);
  }

  // Example chips were pre-wired by the shared wireWidgetChrome to the standard
  // single-pass run; replace each node to drop that listener, then wire our own
  // (apply the preset params, then search).
  for (const chip of Array.from(document.querySelectorAll(".tool-example-chip"))) {
    const fresh = chip.cloneNode(true);
    chip.replaceWith(fresh);
    fresh.addEventListener("click", () => {
      const ex = (ctx.cfg.examples || [])[Number(fresh.dataset.example)];
      if (!ex) return;
      for (const [name, value] of Object.entries(ex.params || {})) {
        const meta = fieldInputs.find((i) => i.name === name);
        if (meta) helpers.applyField(document.getElementById(meta.elementId), value);
      }
      runSearch();
    });
  }

  // Deep-link: scalar params are already prefilled by the shared driver before
  // setup(); if ?url= is present, fetch the remote image into the file input and
  // auto-run (the shared ffmpeg url-loader is skipped by this takeover).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) runSearch();
    });
  }

  return true; // full takeover — the shared ffmpeg wiring must not run
}

// in.<ext> from the uploaded filename (matches site/tool-ffmpeg.js).
function inputNameFor(filename) {
  const dot = filename.lastIndexOf(".");
  const ext = dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "bin";
  return `in.${ext || "bin"}`;
}

function bytesToB64(u8) {
  let s = "";
  const chunk = 0x8000;
  for (let i = 0; i < u8.length; i += chunk) {
    s += String.fromCharCode.apply(null, u8.subarray(i, i + chunk));
  }
  return btoa(s);
}

// Decoded byte length of a base64 string (length is a multiple of 4).
function b64ByteLen(b64) {
  const n = b64.length;
  if (n === 0) return 0;
  const pad = b64.endsWith("==") ? 2 : b64.endsWith("=") ? 1 : 0;
  return (n / 4) * 3 - pad;
}

// Fetch a remote file into the file input. true on success; shows an error and
// returns false when the host blocks cross-origin access.
async function loadUrlIntoFile(url, fileInput, showError) {
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
