// article-to-epub page module — the tool produces a BINARY .epub, so instead of
// dumping the base64 `data:` URL into the text output, render a real Download
// button (reusing the shared #tool-output-download anchor the generator emits
// for text tools) plus a short human summary. Loaded by the shared tool.js via
// the generator's page/custom.js hook.
//
// The web export returns a small JSON envelope on success
// ({ data_url, filename, title, chapters, words, bytes, images_dropped, format })
// or "" when the input is empty (so the idle state is neutral, not an error).
// renderResult/renderError below both return true, so the shared driver never
// falls back to writing the raw value into the output box.

function humanSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(1) + " MB";
}

function plural(n, word) {
  return n + " " + word + (n === 1 ? "" : "s");
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");

  let book = null;
  if (value && String(value).trim().startsWith("{")) {
    try {
      book = JSON.parse(value);
    } catch (e) {
      book = null;
    }
  }

  // Empty input (web export returns "") → neutral idle prompt, no download.
  if (!book || !book.data_url) {
    out.textContent = value
      ? String(value)
      : "Paste an article above — your EPUB download will appear here.";
    if (dl) dl.hidden = true;
    return true;
  }

  const parts = [
    `“${book.title}”`,
    plural(book.chapters, "chapter"),
    plural(book.words, "word"),
    humanSize(book.bytes),
    `read as ${book.format}`,
  ];
  if (book.images_dropped > 0) {
    parts.push(`${plural(book.images_dropped, "image")} dropped (alt text kept)`);
  }
  out.textContent = `EPUB ready — ${parts.join(" · ")}. Click “Download .epub”.`;

  if (dl) {
    dl.href = book.data_url;
    dl.setAttribute("download", book.filename);
    dl.textContent = "Download .epub";
    dl.title = "Download " + book.filename;
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
