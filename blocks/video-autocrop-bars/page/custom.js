// video-autocrop-bars page module — autocrop is a TWO-pass ffmpeg flow (detect
// via cropdetect reading the LOG, then crop), which the generic single-pass
// driver can't express. So this module fully owns the wiring (setup returns
// true), mirroring the chat/CLI block: pass 1 runs the shared wasm's
// detect_argv plan and keeps ffmpeg's log; the shared core (mod.crop_plan)
// parses it and decides crop / no-bars / error; pass 2 runs the crop encode.
// Same takeover shape as video-target-filesize-encoder's custom.js.
//
// Loaded by the shared tool.js via the generator's page/custom.js hook.

import { ffmpegExec } from "./ffmpeg.js";

export function setup(ctx) {
  const { mod, out, helpers, fileInput, fieldInputs, cfg } = ctx;
  const media = document.getElementById("tool-output-media");
  const dl = document.getElementById("tool-output-download");
  if (!mod || !fileInput || !media || !dl) return false; // fall back to shared flow

  const fieldEl = (name) => {
    const meta = fieldInputs.find((i) => i.name === name);
    return meta ? document.getElementById(meta.elementId) : null;
  };
  const thresholdEl = fieldEl("threshold");
  const roundEl = fieldEl("round");

  // Overlapping runs race (ffmpeg times vary); only the newest ticket may paint.
  let runSeq = 0;

  function fail(seq, message) {
    if (seq !== runSeq) return;
    out.classList.add("error");
    out.textContent = message;
  }

  async function runAutocrop() {
    const seq = ++runSeq;
    const file = fileInput.files && fileInput.files[0];
    if (!file) return; // nothing to do until a file is chosen

    const rawThreshold = thresholdEl && thresholdEl.value !== "" ? Number(thresholdEl.value) : NaN;
    const round = (roundEl && roundEl.value) || "2";

    out.classList.remove("error");
    out.textContent = "Detecting black bars…";
    media.hidden = true;
    dl.hidden = true;

    const inName = inputNameFor(file.name);
    const bytesB64 = bytesToB64(new Uint8Array(await file.arrayBuffer()));
    if (seq !== runSeq) return;
    const inputsJson = JSON.stringify([{ name: inName, bytes_b64: bytesB64 }]);

    // Pass 1 — cropdetect (no output file; the result is the log).
    let detectPlan;
    try {
      detectPlan = mod.detect_argv(rawThreshold, round, inName);
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "invalid arguments");
    }
    let detectResp;
    try {
      detectResp = await ffmpegExec(JSON.stringify(detectPlan.argv), inputsJson, detectPlan.out_name);
    } catch (e) {
      return fail(seq, (e && e.message) || "bar detection failed");
    }
    if (seq !== runSeq) return;
    if (detectResp.exit_code !== 0) {
      const snippet = (detectResp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
      return fail(seq, snippet);
    }

    // Decision — shared core parses the log (crop / no-bars / clear error).
    let plan;
    try {
      plan = mod.crop_plan(detectResp.log || "", inName);
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "could not read detection output");
    }
    if (plan.no_bars) {
      // Friendly outcome, not an error: the video is already full picture.
      out.classList.remove("error");
      out.textContent =
        `No black bars detected — the ${plan.in_w}×${plan.in_h} frame is already full ` +
        `picture. If the bars are dark grey rather than black, raise the threshold and re-run.`;
      return;
    }

    // Pass 2 — crop + re-encode.
    out.textContent = `Bars found — cropping ${plan.in_w}×${plan.in_h} → ${plan.w}×${plan.h}…`;
    let cropResp;
    try {
      cropResp = await ffmpegExec(JSON.stringify(plan.argv), inputsJson, plan.out_name);
    } catch (e) {
      return fail(seq, (e && e.message) || "cropping failed");
    }
    if (seq !== runSeq) return;
    if (cropResp.exit_code !== 0 || !cropResp.output_b64) {
      const snippet = (cropResp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
      return fail(seq, snippet);
    }

    const mime = mimeForOutput(plan.out_name);
    const dataUrl = `data:${mime};base64,${cropResp.output_b64}`;
    media.src = dataUrl;
    media.hidden = false;
    dl.href = dataUrl;
    dl.download = plan.out_name;
    dl.hidden = false;
    out.classList.remove("error");
    out.textContent =
      `Removed bars: ${plan.in_w}×${plan.in_h} → ${plan.w}×${plan.h} ` +
      `(crop offset x=${plan.x}, y=${plan.y}).`;
  }

  // Wire inputs. `change` only (not `input`) so the slider drag doesn't launch
  // a detect+encode per pixel; the file input change and each committed field
  // change re-run.
  fileInput.addEventListener("change", runAutocrop);
  for (const el of [thresholdEl, roundEl]) {
    if (el) el.addEventListener("change", runAutocrop);
  }

  // Example chips were pre-wired by the shared wireWidgetChrome to the standard
  // single-pass run; replace each node to drop that listener, then wire our own
  // (apply the preset params, then run the two-pass flow).
  for (const chip of Array.from(document.querySelectorAll(".tool-example-chip"))) {
    const fresh = chip.cloneNode(true);
    chip.replaceWith(fresh);
    fresh.addEventListener("click", () => {
      const ex = (cfg.examples || [])[Number(fresh.dataset.example)];
      if (!ex) return;
      for (const [name, value] of Object.entries(ex.params || {})) {
        const meta = fieldInputs.find((i) => i.name === name);
        if (meta) helpers.applyField(document.getElementById(meta.elementId), value);
      }
      runAutocrop();
    });
  }

  // Deep-link: scalar params are already prefilled by the shared driver before
  // setup(); if ?url= is present, fetch the remote video into the file input and
  // auto-run (the shared ffmpeg url-loader is skipped by this takeover).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) runAutocrop();
    });
  }

  return true; // full takeover — the shared ffmpeg wiring must not run
}

// in.<ext> from the uploaded filename (matches the shared tool-ffmpeg.js).
function inputNameFor(filename) {
  const dot = filename.lastIndexOf(".");
  const ext = dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "bin";
  return `in.${ext || "bin"}`;
}

// MIME for the produced file, from its extension (subset of tool-ffmpeg.js's
// table — autocrop outputs follow the h264_out_ext container rule).
function mimeForOutput(outName) {
  const dot = outName.lastIndexOf(".");
  const ext = dot >= 0 ? outName.slice(dot + 1).toLowerCase() : "";
  return (
    { mp4: "video/mp4", m4v: "video/mp4", mov: "video/quicktime", mkv: "video/x-matroska" }[ext] ||
    "video/mp4"
  );
}

function bytesToB64(u8) {
  let s = "";
  const chunk = 0x8000;
  for (let i = 0; i < u8.length; i += chunk) {
    s += String.fromCharCode.apply(null, u8.subarray(i, i + chunk));
  }
  return btoa(s);
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
