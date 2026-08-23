// midi-chord-progression-generator page module — wasm returns a JSON envelope
// with a data:audio/midi URL. Render a human summary plus a Download button.

function humanSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(1) + " MB";
}

function describe(p) {
  const lines = [];
  lines.push(p.summary || `${p.chords || 0} chord(s), ${p.notes || 0} note(s).`);
  if (p.lowest && p.highest) lines.push(`Range: ${p.lowest} to ${p.highest}.`);
  if (typeof p.seconds === "number") lines.push(`Playing time: ${p.seconds.toFixed(1)} s.`);
  lines.push(`File: ${p.filename || "chord-progression.mid"} (${humanSize(p.bytes || 0)}).`);
  if (p.detail) {
    lines.push("");
    lines.push(p.detail);
  }
  lines.push("");
  lines.push("Click “Download .mid” to save it, then open it in any DAW or notation app.");
  return lines.join("\n");
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");

  if (!value) {
    out.textContent = "Enter a chord progression above — your .mid download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  let payload = null;
  try {
    payload = JSON.parse(String(value));
  } catch (_) {
    payload = null;
  }
  if (!payload || typeof payload.data_url !== "string") {
    out.textContent = String(value);
    if (dl) dl.hidden = true;
    return true;
  }

  const filename = payload.filename || "chord-progression.mid";
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
