// csv-to-xlsx page module — the tool produces a BINARY .xlsx, so instead of
// dumping the base64 `data:` URL into the text output, render a real Download
// button (reusing the shared #tool-output-download anchor the generator emits
// for text tools) plus a short human summary. Loaded by the shared tool.js via
// the generator's page/custom.js hook.
//
// The web export returns the workbook as a `data:<xlsx-mime>;base64,…` URL on
// success, or "" when the input is empty (so the idle state is neutral, not an
// error). renderResult/renderError below both return true, so the shared driver
// never falls back to writing the raw value into the output box.

const FORBIDDEN_SHEET_CHARS = /[[\]:*?/\\]/g;

// Mirror the core's sheet-name sanitizer so the download filename matches the
// worksheet tab name the wasm actually wrote.
function sheetFilename() {
  const el = document.getElementById("in-sheet_name");
  let name = ((el && el.value) || "").replace(FORBIDDEN_SHEET_CHARS, "_").trim();
  if (!name) name = "Sheet1";
  name = Array.from(name).slice(0, 31).join("");
  return name + ".xlsx";
}

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
      : "Paste some table data above — your .xlsx download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  const filename = sheetFilename();
  out.textContent = `Workbook ready (${humanSize(approxBytes(value))}). Click “Download .xlsx”.`;
  if (dl) {
    dl.href = value;
    dl.setAttribute("download", filename);
    dl.textContent = "Download .xlsx";
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
