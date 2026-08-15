// midi-track-splitter page module — the tool's real output is a SET of binary
// Standard MIDI Files, so the web export returns a JSON document (a summary,
// the parts table, and one `data:audio/midi;base64,…` URL per part) instead of
// text. This module turns that document into a readable parts table with a
// Download button on every row. Loaded by the shared tool.js via cfg.custom.
//
// The export returns "" for empty input (so the idle state is neutral rather
// than a red error). renderResult/renderError both return true, so the shared
// driver never falls back to dumping the raw JSON into the output box.
// Everything is built with DOM nodes, never innerHTML — the part names come
// from the uploaded file.

function humanSize(n) {
  if (n < 1024) return n + " B";
  if (n < 1024 * 1024) return (n / 1024).toFixed(1) + " KB";
  return (n / (1024 * 1024)).toFixed(1) + " MB";
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// "channel 1" / "channels 1, 10" — the parts table shows where a part came from.
function describeChannels(channels) {
  if (!Array.isArray(channels) || channels.length === 0) return "—";
  return (channels.length === 1 ? "ch " : "ch ") + channels.join(", ");
}

function buildTable(payload) {
  const wrap = el("div", "split-parts");
  wrap.appendChild(el("p", "split-summary", payload.summary || ""));

  const src = payload.source || {};
  wrap.appendChild(
    el(
      "p",
      "split-source",
      `Source: ${src.format || "?"}, ${src.division || "?"}, ${src.tempo_bpm || 0} BPM, ` +
        `${src.tracks || 0} track(s), ${src.notes || 0} note(s).`
    )
  );

  const table = el("table", "split-table");
  const head = el("tr");
  ["#", "Part", "From", "Instrument", "Notes", "Length", "File"].forEach((h) =>
    head.appendChild(el("th", null, h))
  );
  table.appendChild(head);

  (payload.files || []).forEach((f) => {
    const row = el("tr");
    row.appendChild(el("td", null, String(f.index)));
    row.appendChild(el("td", null, f.name || ""));
    row.appendChild(el("td", null, `${f.source || ""} · ${describeChannels(f.channels)}`));
    row.appendChild(el("td", null, f.instrument || "—"));
    row.appendChild(el("td", null, String(f.notes)));
    row.appendChild(el("td", null, `${Number(f.seconds || 0).toFixed(2)} s`));

    const cell = el("td");
    if (f.data_url) {
      const a = el("a", "split-download", `${f.filename} (${humanSize(f.bytes || 0)})`);
      a.href = f.data_url;
      a.setAttribute("download", f.filename);
      a.title = "Download " + f.filename;
      cell.appendChild(a);
    } else {
      // `output = "list"` is the preview pass: names, no bytes.
      cell.appendChild(el("span", "split-noname", f.filename));
    }
    row.appendChild(cell);
    table.appendChild(row);
  });

  wrap.appendChild(table);
  if (payload.files && payload.files.length && payload.files[0].data_url) {
    wrap.appendChild(
      el(
        "p",
        "split-hint",
        "Each file is a complete .mid — download the parts you need and drop them into any DAW or notation app."
      )
    );
  }
  return wrap;
}

export function renderResult(value, ctx) {
  const { out } = ctx;
  const dl = document.getElementById("tool-output-download");
  out.classList.remove("error");
  if (dl) dl.hidden = true;

  if (!value) {
    out.textContent =
      "Paste a MIDI file above as base64 or hex — one downloadable .mid per track will appear here.";
    return true;
  }

  let payload = null;
  try {
    payload = JSON.parse(String(value));
  } catch (e) {
    payload = null;
  }
  // Anything that isn't the expected document is shown as-is rather than
  // swallowed, so a future export change is visible instead of silent.
  if (!payload || !Array.isArray(payload.files)) {
    out.textContent = String(value);
    return true;
  }

  out.textContent = "";
  out.appendChild(buildTable(payload));
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
