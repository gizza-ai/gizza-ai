// video-speech-segment-detector page module — the tool's OUTPUT IS TEXT (a
// timestamps report), which the generic ffmpeg driver can't express (it
// requires a media output). So this module fully owns the wiring (setup
// returns true), mirroring video-autocrop-bars' takeover shape: pass 1 runs
// the shared wasm's detect_argv plan (silencedetect, no output file) and keeps
// ffmpeg's LOG; the shared core (mod.segments_report) parses it, segments the
// timeline, and renders the report/CSV/SRT/Audacity text, which lands in the
// text output element with a Download link (data: URL, mime from the format).
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
  const thresholdEl = fieldEl("threshold_db");
  const minSilenceEl = fieldEl("min_silence");
  const minSpeechEl = fieldEl("min_speech");
  const padEl = fieldEl("pad");
  const voiceBandEl = fieldEl("voice_band");
  const segmentsEl = fieldEl("segments");
  const outputEl = fieldEl("output");

  // The report is preformatted text — keep columns aligned.
  out.style.fontFamily = "ui-monospace, monospace";

  // Overlapping runs race (ffmpeg times vary); only the newest ticket may paint.
  let runSeq = 0;

  function fail(seq, message) {
    if (seq !== runSeq) return;
    out.classList.add("error");
    out.textContent = message;
  }

  const num = (el) => (el && el.value !== "" ? Number(el.value) : NaN);

  async function runDetect() {
    const seq = ++runSeq;
    const file = fileInput.files && fileInput.files[0];
    if (!file) return; // nothing to do until a file is chosen

    out.classList.remove("error");
    out.textContent = "Detecting speech…";
    media.hidden = true; // never used — the result is text
    dl.hidden = true;

    const inName = inputNameFor(file.name);
    const bytesB64 = bytesToB64(new Uint8Array(await file.arrayBuffer()));
    if (seq !== runSeq) return;
    const inputsJson = JSON.stringify([{ name: inName, bytes_b64: bytesB64 }]);

    // Pass 1 — silencedetect (no output file; the result is the log).
    let plan;
    try {
      plan = mod.detect_argv(
        num(thresholdEl),
        num(minSilenceEl),
        voiceBandEl ? helpers.readField(voiceBandEl) : "true",
        inName
      );
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "invalid arguments");
    }
    let resp;
    try {
      resp = await ffmpegExec(JSON.stringify(plan.argv), inputsJson, plan.out_name);
    } catch (e) {
      return fail(seq, (e && e.message) || "speech detection failed");
    }
    if (seq !== runSeq) return;
    if (resp.exit_code !== 0) {
      return fail(seq, mod.error_message(resp.log || ""));
    }

    // Segment + render — shared core parses the log and formats the output.
    let report;
    try {
      report = mod.segments_report(
        resp.log || "",
        num(minSpeechEl),
        num(padEl),
        (segmentsEl && segmentsEl.value) || "both",
        (outputEl && outputEl.value) || "report"
      );
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "could not read detection output");
    }
    out.classList.remove("error");
    out.textContent = report.text;
    dl.href = `data:${report.mime};charset=utf-8,${encodeURIComponent(report.text)}`;
    dl.download = report.filename;
    dl.hidden = false;
  }

  // Wire inputs. `change` only (not `input`) so the slider drag doesn't launch
  // a detect run per pixel; the file input change and each committed field
  // change re-run.
  fileInput.addEventListener("change", runDetect);
  for (const el of [thresholdEl, minSilenceEl, minSpeechEl, padEl, voiceBandEl, segmentsEl, outputEl]) {
    if (el) el.addEventListener("change", runDetect);
  }

  // Example chips were pre-wired by the shared wireWidgetChrome to the standard
  // single-pass run; replace each node to drop that listener, then wire our own
  // (apply the preset params, then run the detect flow).
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
      runDetect();
    });
  }

  // Deep-link: scalar params are already prefilled by the shared driver before
  // setup(); if ?url= is present, fetch the remote video into the file input and
  // auto-run (the shared ffmpeg url-loader is skipped by this takeover).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) runDetect();
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
