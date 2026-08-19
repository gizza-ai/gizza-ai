// guitar-tab-to-midi page module — the tool's real output is a BINARY Standard
// MIDI File, so the web export returns a small JSON envelope (human summary +
// note statistics + a `data:audio/midi;base64,…` URL) instead of text. This
// module turns that envelope into a readable summary plus a real Download
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

// Build the multi-line summary shown in the output box. Every field comes from
// the envelope the wasm produced, so the page can't disagree with the file.
function describe(p) {
  const lines = [];
  lines.push(
    `${p.notes} note${p.notes === 1 ? "" : "s"} from ${p.staves} stave${
      p.staves === 1 ? "" : "s"
    } × ${p.strings} string${p.strings === 1 ? "" : "s"}.`
  );
  if (p.tuning) lines.push(`Tuning: ${p.tuning}${p.tuning_notes ? ` (${p.tuning_notes})` : ""}.`);
  if (p.lowest && p.highest) lines.push(`Range: ${p.lowest} to ${p.highest}.`);
  if (typeof p.seconds === "number") lines.push(`Playing time: ${p.seconds.toFixed(1)} s.`);
  lines.push(`File: ${p.filename || "guitar-tab.mid"} (${humanSize(p.bytes || 0)}).`);
  lines.push("Click “Download .mid” to save it, then open it in any DAW or notation app.");
  return lines.join("\n");
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");

  if (!value) {
    out.textContent = "Paste some tablature above — your .mid download will appear here.";
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

  const filename = payload.filename || "guitar-tab.mid";
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
