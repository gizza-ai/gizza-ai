// midi-tempo-change page module — the tool's real output is a BINARY Standard
// MIDI File, so the web export returns a small JSON envelope (the numbers
// describing the change plus a `data:audio/midi;base64,…` URL) instead of text.
// This module turns that envelope into a readable summary plus a real Download
// button, reusing the shared #tool-output-download anchor the generator emits
// for `format = "text"` pages. Loaded by the shared tool.js via cfg.custom.
//
// The export returns "" for empty input (so the idle state is neutral rather
// than a red error). renderResult/renderError both return true, so the shared
// driver never falls back to dumping the raw JSON into the output box.

function humanSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(1) + " MB";
}

function plural(n, word) {
  return `${n} ${word}${n === 1 ? "" : "s"}`;
}

// Build the multi-line summary shown in the output box. Every field comes from
// the envelope the wasm produced, so the page can't disagree with the file.
function describe(p) {
  const lines = [];
  lines.push(
    `Tempo: ${p.original_bpm.toFixed(2)} → ${p.new_bpm.toFixed(2)} BPM (${p.ratio.toFixed(3)}×).`
  );
  lines.push(`Tempo events: ${p.tempo_events_in} in → ${p.tempo_events_out} out.`);
  lines.push(
    `${plural(p.tracks, "track")}, ${p.ppq} PPQ, ${plural(p.notes, "note")} unchanged.`
  );
  lines.push(
    `Playing time: ${p.seconds_before.toFixed(3)} s → ${p.seconds_after.toFixed(3)} s.`
  );
  if (p.flattened) lines.push("Tempo map flattened to one constant tempo.");
  if (p.keep_duration)
    lines.push("Note positions rescaled, so the playing time is unchanged.");
  lines.push(`File: ${p.filename || "tempo-changed.mid"} (${humanSize(p.bytes || 0)}).`);
  lines.push("Click “Download .mid” to save it, then open it in any DAW or notation app.");
  return lines.join("\n");
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");

  if (!value) {
    out.textContent =
      "Paste a MIDI file above as base64 or hex — your retimed .mid download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  let payload = null;
  try {
    payload = JSON.parse(String(value));
  } catch (e) {
    payload = null;
  }
  // Anything that isn't the expected envelope is shown as-is rather than
  // swallowed, so a future export change is visible instead of silent.
  if (!payload || typeof payload.data_url !== "string") {
    out.textContent = String(value);
    if (dl) dl.hidden = true;
    return true;
  }

  const filename = payload.filename || "tempo-changed.mid";
  out.textContent = describe(payload);
  if (dl) {
    dl.href = payload.data_url;
    dl.setAttribute("download", filename);
    dl.textContent = "Download .mid";
    dl.title = "Download " + filename;
    dl.hidden = false;
  }
  return true;
}

export function renderError(message, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  if (dl) dl.hidden = true;
  out.classList.add("error");
  out.textContent = message;
  return true;
}
