// markdown-to-pptx page module — the tool produces a BINARY .pptx, so instead of
// dumping the base64 `data:` URL into the text output, render a real Download
// button (reusing the shared #tool-output-download anchor the generator emits
// for text tools) plus a short human summary. Loaded by the shared tool.js via
// the generator's page/custom.js hook.
//
// The web export returns the deck as a `data:<pptx-mime>;base64,…` URL on
// success, or "" when the input is empty (so the idle state is neutral, not an
// error). renderResult/renderError below both return true, so the shared driver
// never falls back to writing the raw value into the output box.

// Decoded byte length of a base64 `data:` URL (for the size hint).
function approxBytes(dataUrl) {
  const i = dataUrl.indexOf("base64,");
  if (i < 0) return 0;
  const b64 = dataUrl.slice(i + 7);
  const padding = b64.endsWith("==") ? 2 : b64.endsWith("=") ? 1 : 0;
  return Math.max(0, Math.floor((b64.length * 3) / 4) - padding);
}

function humanSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(1) + " MB";
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");

  // Empty input (web export returns "") → neutral idle prompt, no download.
  if (!value || !String(value).startsWith("data:")) {
    out.textContent = value
      ? String(value)
      : "Paste a Markdown outline above — your .pptx download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  out.textContent = `Presentation ready (${humanSize(approxBytes(value))}). Click “Download .pptx”.`;
  if (dl) {
    dl.href = value;
    dl.setAttribute("download", "presentation.pptx");
    dl.textContent = "Download .pptx";
    dl.title = "Download presentation.pptx";
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
