// markdown-to-docx page module — the web export returns a binary DOCX as a
// data: URL. Render that value as a Download button and concise status message.

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

  if (!value || !String(value).startsWith("data:")) {
    out.textContent = value
      ? String(value)
      : "Paste Markdown above — your .docx download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  out.textContent = `Document ready (${humanSize(approxBytes(value))}). Click “Download .docx”.`;
  if (dl) {
    dl.href = value;
    dl.setAttribute("download", "document.docx");
    dl.textContent = "Download .docx";
    dl.title = "Download document.docx";
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
