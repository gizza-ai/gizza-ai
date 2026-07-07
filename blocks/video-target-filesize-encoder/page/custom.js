// video-target-filesize-encoder page module — hitting a target FILE SIZE needs
// the clip DURATION to compute the bitrate, which the generic single-pass ffmpeg
// driver doesn't supply. So this tool fully owns its wiring (setup returns true):
// it reads <video>.duration from the uploaded file, asks the shared wasm
// (mod.build_argv) for the ffmpeg argv, runs ONE encode via ffmpegExec, and
// renders the MP4. This mirrors the chat/CLI block's probe-then-encode flow
// (there the duration comes from the `ffmpeg -i` log; here from the browser).
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
  const targetEl = fieldEl("target_mb");
  const audioEl = fieldEl("audio_kbps");
  const scaleEl = fieldEl("scale");

  // Overlapping runs race (ffmpeg times vary); only the newest ticket may paint.
  let runSeq = 0;

  // Read the clip duration (seconds) from a throwaway <video> element. Fast and
  // decode-free, but a browser without the input codec (e.g. Chromium lacks
  // H.264) returns null — the caller then falls back to the ffmpeg probe below.
  function videoElementDuration(file) {
    return new Promise((resolve) => {
      const v = document.createElement("video");
      v.preload = "metadata";
      const url = URL.createObjectURL(file);
      const done = (d) => {
        URL.revokeObjectURL(url);
        resolve(isFinite(d) && d > 0 ? d : null);
      };
      v.onloadedmetadata = () => done(v.duration);
      v.onerror = () => done(null);
      // Don't hang the whole encode if metadata never arrives.
      setTimeout(() => done(v.duration), 4000);
      v.src = url;
    });
  }

  // Codec-agnostic fallback: ask ffmpeg itself for the Duration line (mirrors the
  // chat/CLI block's probe pass). Works for any codec ffmpeg.wasm can decode,
  // regardless of the browser's native <video> support.
  async function ffmpegProbeDuration(inName, inputsJson) {
    let resp;
    try {
      resp = await ffmpegExec(JSON.stringify(["-i", inName, "-f", "null", "-"]), inputsJson, "probe.null");
    } catch (e) {
      return null;
    }
    const m = /Duration:\s*(\d+):(\d\d):(\d\d(?:\.\d+)?)/.exec(resp.log || "");
    if (!m) return null;
    const d = Number(m[1]) * 3600 + Number(m[2]) * 60 + Number(m[3]);
    return isFinite(d) && d > 0 ? d : null;
  }

  async function runEncode() {
    const seq = ++runSeq;
    const file = fileInput.files && fileInput.files[0];
    if (!file) return; // nothing to do until a file is chosen

    const targetMb = parseFloat(targetEl && targetEl.value);
    if (!isFinite(targetMb) || targetMb <= 0) {
      out.classList.remove("error");
      out.textContent = "Enter a target size in MB, then choose a video.";
      return;
    }
    const audio = (audioEl && audioEl.value) || "128";
    const scale = (scaleEl && scaleEl.value) || "keep";

    out.classList.remove("error");
    out.textContent = "Reading clip length…";
    media.hidden = true;
    dl.hidden = true;

    const inName = inputNameFor(file.name);
    const bytesB64 = bytesToB64(new Uint8Array(await file.arrayBuffer()));
    if (seq !== runSeq) return;
    const inputsJson = JSON.stringify([{ name: inName, bytes_b64: bytesB64 }]);

    // Duration: fast <video> path first, ffmpeg probe as a codec-agnostic fallback.
    let duration = await videoElementDuration(file);
    if (seq !== runSeq) return;
    if (!duration) duration = await ffmpegProbeDuration(inName, inputsJson);
    if (seq !== runSeq) return;
    if (!duration) {
      out.classList.add("error");
      out.textContent = "Could not read the clip length — the video may be corrupt or an unsupported format.";
      return;
    }

    let plan;
    try {
      plan = mod.build_argv(targetMb, duration, audio, scale, inName);
    } catch (e) {
      if (seq === runSeq) {
        out.classList.add("error");
        out.textContent = typeof e === "string" ? e : (e && e.message) || "invalid arguments";
      }
      return;
    }

    out.textContent = "Encoding…";
    let resp;
    try {
      resp = await ffmpegExec(JSON.stringify(plan.argv), inputsJson, plan.out_name);
    } catch (e) {
      if (seq === runSeq) {
        out.classList.add("error");
        out.textContent = (e && e.message) || "encoding failed";
      }
      return;
    }
    if (seq !== runSeq) return;
    if (resp.exit_code !== 0 || !resp.output_b64) {
      const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
      out.classList.add("error");
      out.textContent = snippet;
      return;
    }

    const dataUrl = `data:video/mp4;base64,${resp.output_b64}`;
    media.src = dataUrl;
    media.hidden = false;
    dl.href = dataUrl;
    dl.download = plan.out_name;
    dl.hidden = false;
    const outMb = b64ByteLen(resp.output_b64) / (1024 * 1024);
    const fit = outMb <= targetMb;
    const scaleNote = scale === "keep" ? "" : `, capped to ${scale}p`;
    out.classList.remove("error");
    out.textContent = fit
      ? `Done — ${outMb.toFixed(2)} MB (target ${targetMb} MB${scaleNote}).`
      : `Smallest reachable was ${outMb.toFixed(2)} MB, still over the ${targetMb} MB target — lower the resolution or set audio to none.`;
  }

  // Wire inputs. `change` only (not `input`) so we don't launch a whole encode on
  // every keystroke; the file input change and each committed field change re-run.
  fileInput.addEventListener("change", runEncode);
  for (const el of [targetEl, audioEl, scaleEl]) {
    if (el) el.addEventListener("change", runEncode);
  }

  // Example chips were pre-wired by the shared wireWidgetChrome to the standard
  // single-pass run; replace each node to drop that listener, then wire our own
  // (apply the preset params, then encode).
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
      runEncode();
    });
  }

  // Deep-link: scalar params are already prefilled by the shared driver before
  // setup(); if ?url= is present, fetch the remote video into the file input and
  // auto-run (the shared ffmpeg url-loader is skipped by this takeover).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) runEncode();
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
