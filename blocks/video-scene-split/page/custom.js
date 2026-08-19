// video-scene-split page module — splitting by scene is a MULTI-pass ffmpeg flow
// (detect the shot boundaries by reading the LOG, then one extract pass per
// scene) producing MANY outputs, which the generic single-pass driver can't
// express. So this module fully owns the wiring (setup returns true), mirroring
// the chat/CLI block: pass 1 runs the shared wasm's detect_argv plan and keeps
// ffmpeg's log; the shared core (mod.scene_plan) parses it into scene windows
// with a ready-made argv each; then every scene is encoded and listed as its own
// download, alongside the scenes.csv timing table.
//
// Same takeover shape as video-autocrop-bars' custom.js.
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
  const minSceneEl = fieldEl("min_scene");
  const modeEl = fieldEl("mode");
  const crfEl = fieldEl("crf");
  const presetEl = fieldEl("preset");
  const keepAudioEl = fieldEl("keep_audio");

  // The per-clip download list + the CSV link live below the single-file
  // download anchor the template already renders.
  const clipList = document.createElement("ul");
  clipList.id = "scene-clip-list";
  clipList.className = "tool-scene-clips";
  clipList.hidden = true;
  dl.insertAdjacentElement("afterend", clipList);

  // Overlapping runs race (ffmpeg times vary); only the newest ticket may paint.
  let runSeq = 0;
  let objectUrls = [];

  function resetOutputs() {
    for (const u of objectUrls) URL.revokeObjectURL(u);
    objectUrls = [];
    clipList.replaceChildren();
    clipList.hidden = true;
    media.hidden = true;
    dl.hidden = true;
  }

  function fail(seq, message) {
    if (seq !== runSeq) return;
    resetOutputs();
    out.classList.add("error");
    out.textContent = message;
  }

  // Number from a field, NaN when left empty (the wasm then applies the default).
  const numOf = (el) => (el && el.value !== "" ? Number(el.value) : NaN);

  async function runSplit() {
    const seq = ++runSeq;
    const file = fileInput.files && fileInput.files[0];
    if (!file) return; // nothing to do until a file is chosen

    const threshold = numOf(thresholdEl);
    const minScene = numOf(minSceneEl);
    const mode = (modeEl && modeEl.value) || "reencode";
    const crf = numOf(crfEl);
    const preset = (presetEl && presetEl.value) || "veryfast";
    // Checkboxes marshal as "true"/"false" (see tool-ffmpeg.js readField).
    const keepAudio = keepAudioEl && keepAudioEl.checked ? "true" : "false";

    out.classList.remove("error");
    out.textContent = "Detecting scene changes…";
    resetOutputs();

    const inName = inputNameFor(file.name);
    const bytesB64 = bytesToB64(new Uint8Array(await file.arrayBuffer()));
    if (seq !== runSeq) return;
    const inputsJson = JSON.stringify([{ name: inName, bytes_b64: bytesB64 }]);

    // Pass 1 — scene detection (no output file; the result is the log).
    let detectPlan;
    try {
      detectPlan = mod.detect_argv(threshold, inName);
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "invalid arguments");
    }
    let detectResp;
    try {
      detectResp = await ffmpegExec(JSON.stringify(detectPlan.argv), inputsJson, detectPlan.out_name);
    } catch (e) {
      return fail(seq, (e && e.message) || "scene detection failed");
    }
    if (seq !== runSeq) return;
    if (detectResp.exit_code !== 0) {
      const snippet = (detectResp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
      return fail(seq, snippet);
    }

    // Decision — the shared core turns the log into scene windows (or errors).
    let plan;
    try {
      plan = mod.scene_plan(
        detectResp.log || "",
        threshold,
        minScene,
        mode,
        crf,
        preset,
        keepAudio,
        inName,
        file.name
      );
    } catch (e) {
      return fail(seq, typeof e === "string" ? e : (e && e.message) || "could not read detection output");
    }
    if (plan.single) {
      // Friendly outcome, not an error: one scene means nothing to split.
      resetOutputs();
      out.classList.remove("error");
      out.textContent =
        `No scene changes detected at sensitivity ${plan.threshold} — the ` +
        `${fmtSeconds(plan.duration)}s video reads as a single shot. Lower the sensitivity ` +
        `(e.g. 0.15) for soft or graded transitions, or reduce the shortest-scene setting ` +
        `(${plan.min_scene}s) if the cuts are closer together than that.`;
      return;
    }

    // Pass 2..N — one extract pass per scene.
    const clips = [];
    for (const scene of plan.scenes) {
      out.textContent = `Found ${plan.count} scenes — cutting clip ${scene.index} of ${plan.count}…`;
      let resp;
      try {
        resp = await ffmpegExec(JSON.stringify(scene.argv), inputsJson, scene.out_name);
      } catch (e) {
        return fail(seq, (e && e.message) || `cutting scene ${scene.index} failed`);
      }
      if (seq !== runSeq) return;
      if (resp.exit_code !== 0 || !resp.output_b64) {
        const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
        return fail(seq, `scene ${scene.index}: ${snippet}`);
      }
      clips.push({ scene, dataUrl: `data:${mimeForOutput(scene.out_name)};base64,${resp.output_b64}` });
    }

    // Preview the first clip in the player, and offer it on the main download
    // link; every clip (plus scenes.csv) gets its own row underneath.
    media.src = clips[0].dataUrl;
    media.hidden = false;
    dl.href = clips[0].dataUrl;
    dl.download = clips[0].scene.download_name;
    dl.hidden = false;

    for (const { scene, dataUrl } of clips) {
      const li = document.createElement("li");
      const a = document.createElement("a");
      a.href = dataUrl;
      a.download = scene.download_name;
      a.textContent = scene.download_name;
      li.append(a, document.createTextNode(` — ${fmtSeconds(scene.start)}s to ${fmtSeconds(scene.end)}s (${fmtSeconds(scene.duration)}s)`));
      clipList.append(li);
    }
    const csvUrl = URL.createObjectURL(new Blob([plan.csv], { type: "text/csv" }));
    objectUrls.push(csvUrl);
    const csvLi = document.createElement("li");
    const csvA = document.createElement("a");
    csvA.href = csvUrl;
    csvA.download = "scenes.csv";
    csvA.textContent = "scenes.csv";
    csvLi.append(csvA, document.createTextNode(" — scene timings as a spreadsheet"));
    clipList.append(csvLi);
    clipList.hidden = false;

    out.classList.remove("error");
    out.textContent =
      `Split into ${plan.count} scenes (${fmtSeconds(plan.duration)}s total) at sensitivity ` +
      `${plan.threshold}, shortest scene ${plan.min_scene}s. Previewing scene 1; every clip is ` +
      `listed below.`;
  }

  // Wire inputs. `change` only (not `input`) so a slider drag doesn't launch a
  // detect + N-encode run per pixel.
  fileInput.addEventListener("change", runSplit);
  for (const el of [thresholdEl, minSceneEl, modeEl, crfEl, presetEl, keepAudioEl]) {
    if (el) el.addEventListener("change", runSplit);
  }

  // Example chips were pre-wired by the shared wireWidgetChrome to the standard
  // single-pass run; replace each node to drop that listener, then wire our own.
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
      runSplit();
    });
  }

  // Deep-link: scalar params are already prefilled by the shared driver before
  // setup(); if ?url= is present, fetch the remote video into the file input and
  // auto-run (the shared ffmpeg url-loader is skipped by this takeover).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) runSplit();
    });
  }

  return true; // full takeover — the shared ffmpeg wiring must not run
}

// Trim a float to at most 2 decimals without a trailing ".00" (2 not 2.00).
function fmtSeconds(v) {
  const n = Number(v);
  if (!isFinite(n)) return String(v);
  return String(Math.round(n * 100) / 100);
}

// in.<ext> from the uploaded filename (matches the shared tool-ffmpeg.js).
function inputNameFor(filename) {
  const dot = filename.lastIndexOf(".");
  const ext = dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "bin";
  return `in.${ext || "bin"}`;
}

// MIME for a produced clip, from its extension (subset of tool-ffmpeg.js's
// table — clips are MP4 when re-encoded, or the source container when copied).
function mimeForOutput(outName) {
  const dot = outName.lastIndexOf(".");
  const ext = dot >= 0 ? outName.slice(dot + 1).toLowerCase() : "";
  return (
    {
      mp4: "video/mp4",
      m4v: "video/mp4",
      mov: "video/quicktime",
      mkv: "video/x-matroska",
      webm: "video/webm",
    }[ext] || "video/mp4"
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
