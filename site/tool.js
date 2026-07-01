// Generic standalone-tool driver. Reads window.GIZZA_TOOL (baked by the page
// generator), loads the tool's wasm-bindgen module, wires inputs to the
// exported function, and renders the result. Shared by every tool page (/tools/<slug>/).

import { queryPrefill } from "./query-prefill.js";

const cfg = window.GIZZA_TOOL;
const out = document.getElementById(cfg.output.elementId);

function showResult(value) {
  out.classList.remove("error");
  if (cfg.slug === "timezone-convert") {
    try {
      const data = JSON.parse(value);
      renderTimezoneConvert(data);
      return;
    } catch (e) {
      // If parsing fails, fall through to text display
    }
  } else if (cfg.slug === "age-calculator") {
    try {
      const data = JSON.parse(value);
      renderAgeCalculator(data);
      return;
    } catch (e) {
      // If parsing fails, fall through to text display
    }
  } else if (cfg.slug === "jwt-decode") {
    try {
      const data = JSON.parse(value);
      renderJwtDecode(data);
      return;
    } catch (e) {
      // If parsing fails, fall through to text display
    }
  } else if (cfg.slug === "list-converter") {
    renderListConverterResult(value);
    return;
  }
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
}

function showError(message) {
  // Layout stability: never resize the widget on errors/keystrokes — nothing
  // may jump under the user's cursor. Wide layouts are the tool-widget--wide
  // class (meta.toml `wide = true`), not a JS width override.
  if (cfg.slug === "list-converter") {
    const inInput = document.getElementById("in-input");
    const outputTextarea = document.getElementById("list-conv-output-area");
    if (inInput && !inInput.value.trim()) {
      out.classList.remove("error");
      if (outputTextarea) outputTextarea.value = "";
      const statIn = document.getElementById("list-conv-stat-in");
      const statOut = document.getElementById("list-conv-stat-out");
      const diffContainer = document.getElementById("list-conv-stat-diff-container");
      if (statIn) statIn.textContent = "0";
      if (statOut) statOut.textContent = "0";
      if (diffContainer) diffContainer.style.display = "none";
      return;
    }
    out.classList.remove("error");
    if (outputTextarea) {
      outputTextarea.value = message;
    }
    return;
  }

  out.classList.add("error");
  out.textContent = message;
}


function renderTimezoneConvert(data) {
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "760px";
  }

  out.innerHTML = "";
  out.className = "tz-active-container";

  const grid = document.createElement("div");
  grid.className = "tz-target-grid";

  data.targets.forEach((t) => {
    const card = document.createElement("div");
    card.className = "tz-card";

    const zone = document.createElement("div");
    zone.className = "tz-card-zone";
    zone.textContent = t.to_zone;
    card.appendChild(zone);

    const time = document.createElement("div");
    time.className = "tz-card-time";
    const prettyParts = t.to_pretty.split(" ");
    const timeStr = prettyParts[prettyParts.length - 1];
    time.textContent = timeStr;
    card.appendChild(time);

    const date = document.createElement("div");
    date.className = "tz-card-date";
    date.textContent = prettyParts.slice(0, prettyParts.length - 1).join(" ");
    card.appendChild(date);

    const meta = document.createElement("div");
    meta.className = "tz-card-meta";

    const badgeOffset = document.createElement("span");
    badgeOffset.className = "tz-badge tz-badge-offset";
    badgeOffset.textContent = t.to_offset;
    meta.appendChild(badgeOffset);

    if (t.to_is_dst) {
      const badgeDst = document.createElement("span");
      badgeDst.className = "tz-badge tz-badge-dst";
      badgeDst.textContent = "DST";
      meta.appendChild(badgeDst);
    }

    const diffHours = t.offset_diff_hours;
    const badgeDiff = document.createElement("span");
    badgeDiff.className = "tz-badge tz-badge-diff";
    if (diffHours === 0) {
      badgeDiff.textContent = "Same time";
    } else {
      const sign = diffHours > 0 ? "+" : "";
      badgeDiff.textContent = `${sign}${diffHours}h`;
    }
    meta.appendChild(badgeDiff);
    card.appendChild(meta);

    grid.appendChild(card);
  });
  out.appendChild(grid);

  const planner = document.createElement("div");
  planner.className = "tz-planner-section";

  const title = document.createElement("div");
  title.className = "tz-planner-title";
  title.textContent = `Meeting Planner Grid (Source: ${data.from_zone})`;
  planner.appendChild(title);

  const wrapper = document.createElement("div");
  wrapper.className = "tz-planner-table-wrapper";

  const table = document.createElement("table");
  table.className = "tz-planner-table";

  const thead = document.createElement("thead");
  const headerRow = document.createElement("tr");

  const thSource = document.createElement("th");
  thSource.textContent = `${data.from_zone} (Source)`;
  headerRow.appendChild(thSource);

  data.targets.forEach((t) => {
    const thTarget = document.createElement("th");
    thTarget.textContent = t.to_zone;
    headerRow.appendChild(thTarget);
  });
  thead.appendChild(headerRow);
  table.appendChild(thead);

  const tbody = document.createElement("tbody");
  data.meeting_planner.forEach((slot) => {
    const row = document.createElement("tr");
    row.className = "tz-planner-row";
    
    const inputHour = parseInt(data.from.split("T")[1].split(":")[0], 10);
    if (slot.from_hour === inputHour) {
      row.classList.add("selected");
    }

    const tdSource = document.createElement("td");
    const spanSourceTime = document.createElement("span");
    spanSourceTime.style.fontWeight = "bold";
    spanSourceTime.style.marginRight = "8px";
    spanSourceTime.textContent = slot.from_time;
    tdSource.appendChild(spanSourceTime);

    const badgeSource = document.createElement("span");
    badgeSource.className = `tz-status-badge tz-status-${slot.from_status.toLowerCase()}`;
    badgeSource.textContent = slot.from_status;
    tdSource.appendChild(badgeSource);
    row.appendChild(tdSource);

    slot.targets.forEach((t) => {
      const tdTarget = document.createElement("td");
      const spanTargetTime = document.createElement("span");
      spanTargetTime.style.fontWeight = "bold";
      spanTargetTime.style.marginRight = "8px";
      spanTargetTime.textContent = t.to_time;
      tdTarget.appendChild(spanTargetTime);

      const badgeTarget = document.createElement("span");
      badgeTarget.className = `tz-status-badge tz-status-${t.to_status.toLowerCase()}`;
      badgeTarget.textContent = t.to_status;
      tdTarget.appendChild(badgeTarget);
      row.appendChild(tdTarget);
    });

    row.addEventListener("click", () => {
      tbody.querySelectorAll(".tz-planner-row").forEach((r) => r.classList.remove("selected"));
      row.classList.add("selected");
    });

    tbody.appendChild(row);
  });
  table.appendChild(tbody);
  wrapper.appendChild(table);
  planner.appendChild(wrapper);
  out.appendChild(planner);
}

function formatNumber(v) {
  if (!Number.isFinite(v)) return String(v);
  // Trim float noise without forcing decimals on integers — but only when the
  // *1e12 scaling stays finite. For very large magnitudes (|v| > ~1.8e296) the
  // scaling would overflow to Infinity and misreport a valid finite result, so
  // fall back to the unrounded value there.
  const scaled = Math.round(v * 1e12) / 1e12;
  return Number.isFinite(scaled) ? String(scaled) : String(v);
}

// Read a field element as the string the wasm export expects. A checkbox
// yields "true"/"false" (the wasm side parses booleans from strings); a
// <select>/<input>/<textarea> yields its value.
function readField(el) {
  if (!el) return "";
  if (el.type === "checkbox") return el.checked ? "true" : "false";
  return el.value;
}

// Apply a deep-link prefill value to a field element (checkbox vs value-bearing).
function applyField(el, value) {
  if (!el) return;
  if (el.type === "checkbox") {
    el.checked = ["true", "1", "yes", "on"].includes(String(value).toLowerCase());
  } else {
    el.value = value;
  }
}

// ---- Declarative widget behaviors (defaults / example chips / reset / copy) ----
// Driven by window.GIZZA_TOOL, which the generator bakes from page/meta.toml —
// every page gets these with ZERO per-tool JS. Do not add cfg.slug branches here;
// extend the meta/generator instead (workspace fix-at-root-cause rule).

// Resolve a meta `default` spec: "today"/"now" → the user's local date(-time),
// "local-timezone" → their IANA zone, anything else literal.
function resolveDefault(spec) {
  const pad = (n) => String(n).padStart(2, "0");
  const now = new Date();
  if (spec === "today") {
    return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  }
  if (spec === "now") {
    return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}:${pad(now.getMinutes())}`;
  }
  if (spec === "local-timezone") {
    try {
      return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
    } catch (e) {
      return "UTC";
    }
  }
  return spec;
}

// Apply meta-declared defaults to empty fields. URL-prefilled or already-filled
// fields are left alone unless `force` (the Reset path).
function applyMetaDefaults(force = false) {
  const params = new URLSearchParams(location.search);
  for (const inp of cfg.inputs) {
    if (inp.source !== "field" || !inp.default) continue;
    const el = document.getElementById(inp.elementId);
    if (!el) continue;
    if (!force && (params.has(inp.name) || el.value)) continue;
    applyField(el, resolveDefault(inp.default));
  }
}

// Wire the example chips + Reset + Copy-result chrome the template renders.
// `rerun` recomputes after programmatic input changes (compute for pure tools;
// for ffmpeg tools it is the run fn, a no-op until a file is chosen).
function wireWidgetChrome(rerun) {
  for (const btn of document.querySelectorAll(".tool-example-chip")) {
    btn.addEventListener("click", () => {
      const ex = (cfg.examples || [])[Number(btn.dataset.example)];
      if (!ex) return;
      for (const [name, value] of Object.entries(ex.params || {})) {
        const inp = cfg.inputs.find((i) => i.name === name);
        if (inp) applyField(document.getElementById(inp.elementId), value);
      }
      rerun();
    });
  }

  const reset = document.getElementById("tool-reset");
  if (reset) {
    reset.addEventListener("click", () => {
      for (const inp of cfg.inputs) {
        const el = document.getElementById(inp.elementId);
        if (!el) continue;
        if (inp.source === "file") {
          el.value = "";
          continue;
        }
        if (inp.source !== "field") continue;
        // defaultValue/defaultChecked restore the server-rendered state (the
        // select's default option, the checkbox default, a number's default).
        if (el.type === "checkbox") {
          el.checked = el.defaultChecked;
        } else if (el.tagName === "SELECT") {
          for (const o of el.options) o.selected = o.defaultSelected;
        } else {
          el.value = el.defaultValue;
        }
      }
      applyMetaDefaults(true);
      const media = document.getElementById("tool-output-media");
      const dl = document.getElementById("tool-output-download");
      if (media) media.hidden = true;
      if (dl) dl.hidden = true;
      history.replaceState({}, document.title, location.pathname);
      rerun();
    });
  }

  const copy = document.getElementById("tool-copy-output");
  if (copy) {
    copy.addEventListener("click", async () => {
      const text = (out.textContent || "").trim();
      if (!text) return;
      try {
        await navigator.clipboard.writeText(text);
        copy.classList.add("copied");
        const prev = copy.textContent;
        copy.textContent = "Copied!";
        setTimeout(() => {
          copy.classList.remove("copied");
          copy.textContent = prev;
        }, 1500);
      } catch (e) {
        // Clipboard unavailable (e.g. non-secure context) — button is best-effort.
      }
    });
  }
}

// Collect call args in declared order. "field" → input value; "clock" → now (s).
function gatherArgs() {
  return cfg.inputs.map((inp) => {
    if (inp.source === "clock") return Math.floor(Date.now() / 1000);
    return readField(document.getElementById(inp.elementId));
  });
}

async function main() {
  let mod;
  try {
    mod = await import(cfg.module);
    await mod.default(); // wasm-pack --target web init
  } catch (e) {
    showError("Failed to load tool.");
    return;
  }
  if (cfg.slug === "krutidev-unicode-converter") {
    renderKrutidevUnicode(mod);
    return;
  }
  if (cfg.slug === "list-converter") {
    renderListConverter(mod);
  }
  if (cfg.runtime === "ffmpeg") {
    const { runFfmpeg } = await import("./tool-ffmpeg.js");
    const { ffmpegExec } = await import("./ffmpeg.js");
    const media = document.getElementById("tool-output-media");
    const dl = document.getElementById("tool-output-download");
    if (!media || !dl) {
      // ffmpeg runtime requires a media output (format "image"/"video"/"audio");
      // a misconfigured tool (e.g. runtime=ffmpeg + format=text) has no place to
      // render the result. Fail loudly instead of throwing on a null element.
      showError("tool misconfigured: ffmpeg runtime needs an image/video/audio output");
      return;
    }
    const fileMeta = cfg.inputs.find((i) => i.source === "file");
    const fileInput = fileMeta ? document.getElementById("in-" + fileMeta.name) : null;
    const fieldInputs = cfg.inputs.filter((i) => i.source === "field");

    if (cfg.slug === "image-crop") {
      setupImageCropCustomUI();
    }

    async function run() {
      const file = fileInput && fileInput.files && fileInput.files[0];
      if (!file) return;
      out.textContent = "Processing…";
      out.classList.remove("error");
      media.hidden = true;
      dl.hidden = true;
      // Coerce numeric-looking field values to Number so wasm-bindgen f64 params
      // marshal correctly; leave non-numeric (e.g. "contain") and empty strings
      // as strings — the WASM function handles empty via its own defaults.
      const fieldArgs = fieldInputs.map((i) => {
        const el = document.getElementById(i.elementId);
        const v = el ? el.value : "";
        return v !== "" && !isNaN(Number(v)) ? Number(v) : v;
      });
      const r = await runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs);
      if (r.ok) {
        out.textContent = "";
        media.src = r.dataUrl;
        media.hidden = false;
        dl.href = r.dataUrl;
        dl.download = r.outName;
        dl.hidden = false;
      } else {
        showError(r.error);
      }
    }

    // Deep-link: prefill scalar fields; if ?url= is present, fetch the remote
    // media into the file input and auto-run. Param names == input names.
    const { fields: qpFields, url: qpUrl } = queryPrefill(cfg.inputs, location.search);
    for (const f of qpFields) {
      applyField(document.getElementById(f.elementId), f.value);
    }
    applyMetaDefaults();
    wireWidgetChrome(run);
    async function loadUrlIntoFile(url) {
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

    if (fileInput) {
      if (cfg.slug !== "image-crop") {
        fileInput.addEventListener("change", run);
      }
    }
    for (const i of fieldInputs) {
      const el = document.getElementById(i.elementId);
      if (el) {
        if (cfg.slug === "image-crop") {
          // Only trigger ffmpeg execution on change (drag-end or input blur) to prevent lag
          el.addEventListener("change", run);
        } else {
          el.addEventListener("input", run);
          el.addEventListener("change", run); // <select>/checkbox fire change, not input
        }
      }
    }
    if (qpUrl && fileInput) {
      loadUrlIntoFile(qpUrl).then((ok) => {
        if (ok) run();
      });
    }
    return;
  }

  if (cfg.slug === "timezone-convert") {
    setupTimezoneConvertDefaults();
  } else if (cfg.slug === "jwt-decode") {
    setupJwtDecodeDefaults();
  }

  const fn = mod[cfg.export];

  function compute() {
    try {
      const result = fn(...gatherArgs());
      showResult(result);
    } catch (e) {
      const msg = typeof e === "string" ? e : e && e.message ? e.message : "error";
      // Don't shout at the user for an empty field.
      const hasField = cfg.inputs.some((i) => i.source === "field");
      const empty = hasField && gatherArgs().every((a) => a === "" || a == null);
      if (empty) {
        out.classList.remove("error");
        out.textContent = "";
      } else {
        showError(msg);
      }
    }
  }

  // Deep-link: prefill fields from the URL query, then the initial compute()
  // below auto-runs with those values. Param names == input names.
  for (const f of queryPrefill(cfg.inputs, location.search).fields) {
    applyField(document.getElementById(f.elementId), f.value);
  }
  applyMetaDefaults();
  wireWidgetChrome(compute);

  // Wire field inputs to live recompute.
  for (const inp of cfg.inputs) {
    if (inp.source === "field") {
      const el = document.getElementById(inp.elementId);
      if (el) {
        el.addEventListener("input", compute);
        el.addEventListener("change", compute); // <select>/checkbox fire change, not input
      }
    }
  }

  if (cfg.live) {
    compute();
    setInterval(compute, cfg.intervalMs || 1000);
  } else {
    compute(); // initial (e.g. prefilled / empty state)
  }
}

function setupTimezoneConvertDefaults() {
  // Legacy custom UI: it ships its own Reset, and its dashboard output makes the
  // generic Copy-result copy a text soup — drop the generic chrome until this
  // tool is migrated to the declarative meta.toml controls.
  document.getElementById("tool-reset")?.remove();
  document.getElementById("tool-copy-output")?.remove();
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "760px";
  }

  const dtInput = document.getElementById("in-datetime");
  const fromInput = document.getElementById("in-from");
  const toInput = document.getElementById("in-to");
  
  const dtLabel = document.querySelector('label[for="in-datetime"]');
  const fromLabel = document.querySelector('label[for="in-from"]');
  const toLabel = document.querySelector('label[for="in-to"]');

  const timezones = [
    "UTC", "GMT",
    "Africa/Cairo", "Africa/Johannesburg", "Africa/Lagos", "Africa/Nairobi",
    "America/Anchorage", "America/Argentina/Buenos_Aires", "America/Bogota",
    "America/Chicago", "America/Denver", "America/Halifax", "America/Los_Angeles",
    "America/Mexico_City", "America/New_York", "America/Phoenix", "America/Santiago",
    "America/Sao_Paulo", "America/St_Johns", "America/Toronto", "America/Vancouver",
    "Asia/Bangkok", "Asia/Dubai", "Asia/Hong_Kong", "Asia/Jakarta", "Asia/Jerusalem",
    "Asia/Kabul", "Asia/Kolkata", "Asia/Kathmandu", "Asia/Manila", "Asia/Riyadh",
    "Asia/Seoul", "Asia/Shanghai", "Asia/Singapore", "Asia/Taipei", "Asia/Tashkent",
    "Asia/Tehran", "Asia/Tokyo", "Atlantic/Azores",
    "Australia/Adelaide", "Australia/Brisbane", "Australia/Darwin", "Australia/Hobart",
    "Australia/Melbourne", "Australia/Perth", "Australia/Sydney",
    "Europe/Amsterdam", "Europe/Athens", "Europe/Belgrade", "Europe/Berlin",
    "Europe/Brussels", "Europe/Budapest", "Europe/Copenhagen", "Europe/Dublin",
    "Europe/Helsinki", "Europe/Istanbul", "Europe/Lisbon", "Europe/London",
    "Europe/Madrid", "Europe/Moscow", "Europe/Oslo", "Europe/Paris", "Europe/Prague",
    "Europe/Rome", "Europe/Stockholm", "Europe/Vienna", "Europe/Warsaw", "Europe/Zurich",
    "Pacific/Auckland", "Pacific/Chatham", "Pacific/Fiji", "Pacific/Honolulu"
  ];

  // Create datalist if it doesn't exist
  if (!document.getElementById("tz-list")) {
    const dl = document.createElement("datalist");
    dl.id = "tz-list";
    timezones.forEach((tz) => {
      const opt = document.createElement("option");
      opt.value = tz;
      dl.appendChild(opt);
    });
    document.body.appendChild(dl);
  }

  // Attach datalist to inputs
  if (fromInput) fromInput.setAttribute("list", "tz-list");

  if (dtInput) {
    dtInput.type = "datetime-local";
  }

  // Format current local time: YYYY-MM-DDTHH:MM
  const now = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  const localDateTimeStr = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}T${pad(now.getHours())}:${pad(now.getMinutes())}`;

  // Apply default values if empty and not prefilled by URL query params
  const params = new URLSearchParams(location.search);
  
  if (dtInput && !dtInput.value && !params.has("datetime")) {
    dtInput.value = localDateTimeStr;
  } else if (dtInput && dtInput.value && dtInput.value.includes(" ")) {
    dtInput.value = dtInput.value.replace(" ", "T");
  }
  
  if (fromInput && !fromInput.value && !params.has("from")) {
    try {
      const userTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
      if (userTz) {
        fromInput.value = userTz;
      } else {
        fromInput.value = "America/New_York";
      }
    } catch (e) {
      fromInput.value = "America/New_York";
    }
  }

  if (toInput && !toInput.value && !params.has("to")) {
    toInput.value = "UTC";
  }

  // Rearrange DOM into two-column layout
  if (widget && dtInput && fromInput && toInput) {
    const wrapper = document.createElement("div");
    wrapper.className = "tz-inputs-wrapper";

    const leftCol = document.createElement("div");
    leftCol.className = "tz-left-inputs";
    
    if (dtLabel && dtInput) {
      const group = document.createElement("div");
      group.appendChild(dtLabel);
      group.appendChild(dtInput);
      leftCol.appendChild(group);
    }
    if (fromLabel && fromInput) {
      const group = document.createElement("div");
      group.appendChild(fromLabel);
      group.appendChild(fromInput);
      leftCol.appendChild(group);
    }

    const resetBtn = document.createElement("button");
    resetBtn.type = "button";
    resetBtn.id = "tz-reset-btn";
    resetBtn.className = "tz-reset-btn";
    resetBtn.innerHTML = `<svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg> Reset to Defaults`;
    leftCol.appendChild(resetBtn);

    const rightCol = document.createElement("div");
    rightCol.className = "tz-right-inputs";

    if (toLabel) {
      rightCol.appendChild(toLabel);
    }
    rightCol.appendChild(toInput); // Hidden via CSS

    const tagsContainer = document.createElement("div");
    tagsContainer.className = "tz-tags-list";
    rightCol.appendChild(tagsContainer);

    const addRow = document.createElement("div");
    addRow.className = "tz-input-add-row";

    const searchInput = document.createElement("input");
    searchInput.type = "text";
    searchInput.id = "tz-search-input";
    searchInput.className = "tool-input tz-search-input";
    searchInput.placeholder = "Type & select timezone...";
    searchInput.setAttribute("list", "tz-list");
    searchInput.autocomplete = "off";
    addRow.appendChild(searchInput);

    const addBtn = document.createElement("button");
    addBtn.type = "button";
    addBtn.id = "tz-add-btn";
    addBtn.className = "tz-add-btn";
    addBtn.textContent = "Add";
    addRow.appendChild(addBtn);

    rightCol.appendChild(addRow);
    
    wrapper.appendChild(leftCol);
    wrapper.appendChild(rightCol);

    // Insert wrapper at the top of the widget
    widget.insertBefore(wrapper, widget.firstChild);

    // Tags rendering
    function renderTags(targets) {
      tagsContainer.innerHTML = "";
      if (targets.length === 0) {
        const placeholder = document.createElement("div");
        placeholder.className = "tz-tags-placeholder";
        placeholder.textContent = "No target zones added yet.";
        tagsContainer.appendChild(placeholder);
        return;
      }

      targets.forEach((tz) => {
        const pill = document.createElement("span");
        pill.className = "tz-tag-pill";
        pill.textContent = tz + " ";

        const delBtn = document.createElement("span");
        delBtn.className = "tz-tag-del";
        delBtn.innerHTML = "&times;";
        delBtn.addEventListener("click", () => {
          let current = toInput.value.split(",")
            .map(s => s.trim())
            .filter(s => s.length > 0);
          current = current.filter(t => t !== tz);
          toInput.value = current.join(", ");
          renderTags(current);
          toInput.dispatchEvent(new Event("change"));
        });

        pill.appendChild(delBtn);
        tagsContainer.appendChild(pill);
      });
    }

    // Add timezone handler
    function addTargetTimezone(tzName) {
      tzName = tzName.trim();
      if (!tzName) return;

      const matchedTz = timezones.find(t => t.toLowerCase() === tzName.toLowerCase());
      if (!matchedTz) return;

      let targets = toInput.value.split(",")
        .map(s => s.trim())
        .filter(s => s.length > 0);

      if (!targets.includes(matchedTz)) {
        targets.push(matchedTz);
        toInput.value = targets.join(", ");
        renderTags(targets);
        toInput.dispatchEvent(new Event("change"));
      }
    }

    // Initialize tags
    const initialTargets = toInput.value.split(",")
      .map(s => s.trim())
      .filter(s => s.length > 0);
    renderTags(initialTargets);

    // Wire add button
    addBtn.addEventListener("click", () => {
      addTargetTimezone(searchInput.value);
      searchInput.value = "";
    });

    // Wire enter key on search input
    searchInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        addTargetTimezone(searchInput.value);
        searchInput.value = "";
      }
    });

    // Automatically add if a matching option is entered (e.g. clicked from datalist)
    searchInput.addEventListener("input", () => {
      const val = searchInput.value.trim();
      const matchedTz = timezones.find(t => t.toLowerCase() === val.toLowerCase());
      if (matchedTz && val.length === matchedTz.length) {
        addTargetTimezone(val);
        searchInput.value = "";
      }
    });

    // Wire reset button
    resetBtn.addEventListener("click", () => {
      dtInput.value = localDateTimeStr;
      try {
        const userTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
        fromInput.value = userTz || "America/New_York";
      } catch (e) {
        fromInput.value = "America/New_York";
      }
      toInput.value = "UTC";
      renderTags(["UTC"]);
      
      // Clear search input
      searchInput.value = "";

      // Trigger change to recalculate
      dtInput.dispatchEvent(new Event("input"));
      fromInput.dispatchEvent(new Event("input"));
      toInput.dispatchEvent(new Event("change"));
    });
  }
}

function renderAgeCalculator(data) {
  out.innerHTML = "";
  out.className = "age-active-container";

  // 1. Hero Summary
  const hero = document.createElement("div");
  hero.className = "age-result-hero";

  const heroTitle = document.createElement("div");
  heroTitle.className = "age-result-hero-title";
  heroTitle.textContent = "Exact Age";
  hero.appendChild(heroTitle);

  const heroText = document.createElement("div");
  heroText.className = "age-result-hero-text";

  const yearsSpan = `<span>${data.years}</span> year${data.years === 1 ? '' : 's'}`;
  const monthsSpan = `<span>${data.months}</span> month${data.months === 1 ? '' : 's'}`;
  const daysSpan = `<span>${data.days}</span> day${data.days === 1 ? '' : 's'}`;

  heroText.innerHTML = `${yearsSpan}, ${monthsSpan}, and ${daysSpan} old`;
  hero.appendChild(heroText);
  out.appendChild(hero);

  // 2. Info Cards Grid (Next Birthday, Astrology, Birth Details)
  const infoGrid = document.createElement("div");
  infoGrid.className = "age-info-grid";

  // Card 1: Next Birthday
  const bdayCard = document.createElement("div");
  bdayCard.className = "age-info-card";

  const bdayTitle = document.createElement("div");
  bdayTitle.className = "age-info-card-title";
  bdayTitle.textContent = "Next Birthday";
  bdayCard.appendChild(bdayTitle);

  const bdayValue = document.createElement("div");
  bdayValue.className = "age-info-card-value";
  if (data.days_until_next_birthday === 0) {
    bdayValue.textContent = "Today! 🎉";
  } else {
    bdayValue.textContent = `${data.days_until_next_birthday} Day${data.days_until_next_birthday === 1 ? '' : 's'}`;
  }
  bdayCard.appendChild(bdayValue);

  const bdaySub = document.createElement("div");
  bdaySub.className = "age-info-card-sub";
  bdaySub.textContent = `Date: ${data.next_birthday}`;
  bdayCard.appendChild(bdaySub);

  infoGrid.appendChild(bdayCard);

  // Card 2: Birth Details
  const detailsCard = document.createElement("div");
  detailsCard.className = "age-info-card";

  const detailsTitle = document.createElement("div");
  detailsTitle.className = "age-info-card-title";
  detailsTitle.textContent = "Born On";
  detailsCard.appendChild(detailsTitle);

  const detailsValue = document.createElement("div");
  detailsValue.className = "age-info-card-value";
  detailsValue.textContent = data.day_of_week_born;
  detailsCard.appendChild(detailsValue);

  const detailsSub = document.createElement("div");
  detailsSub.className = "age-info-card-sub";
  detailsSub.textContent = `Birthdate: ${data.birthdate}`;
  detailsCard.appendChild(detailsSub);

  infoGrid.appendChild(detailsCard);

  // Card 3: Zodiac & Astrology
  const zodiacCard = document.createElement("div");
  zodiacCard.className = "age-info-card";

  const zodiacTitle = document.createElement("div");
  zodiacTitle.className = "age-info-card-title";
  zodiacTitle.textContent = "Zodiac & Generation";
  zodiacCard.appendChild(zodiacTitle);

  const zodiacValue = document.createElement("div");
  zodiacValue.className = "age-info-card-value";
  zodiacValue.textContent = `${data.zodiac_sign} / ${data.chinese_zodiac}`;
  zodiacCard.appendChild(zodiacValue);

  const zodiacSub = document.createElement("div");
  zodiacSub.className = "age-info-card-sub";
  zodiacSub.textContent = data.generation;
  zodiacCard.appendChild(zodiacSub);

  infoGrid.appendChild(zodiacCard);
  out.appendChild(infoGrid);

  // 3. Lived Metrics Card
  const livedCard = document.createElement("div");
  livedCard.className = "age-lived-card";

  const livedTitle = document.createElement("div");
  livedTitle.className = "age-lived-card-title";
  livedTitle.textContent = "Lifetime Metrics";
  livedCard.appendChild(livedTitle);

  const livedGrid = document.createElement("div");
  livedGrid.className = "age-lived-grid";

  const addMetric = (val, label) => {
    const item = document.createElement("div");
    item.className = "age-lived-item";
    const valueEl = document.createElement("div");
    valueEl.className = "age-lived-item-value";
    valueEl.textContent = val.toLocaleString();
    const labelEl = document.createElement("div");
    labelEl.className = "age-lived-item-label";
    labelEl.textContent = label;
    item.appendChild(valueEl);
    item.appendChild(labelEl);
    livedGrid.appendChild(item);
  };

  addMetric(data.total_months, "Total Months");
  addMetric(data.total_weeks, "Total Weeks");
  addMetric(data.total_days, "Total Days");
  addMetric(data.total_hours, "Total Hours");
  addMetric(data.total_minutes, "Total Minutes");
  addMetric(data.total_seconds, "Total Seconds");

  livedCard.appendChild(livedGrid);
  out.appendChild(livedCard);
}

function renderKrutidevUnicode(mod) {
  // Legacy dual-pane UI with its own copy/clear buttons — drop the generic
  // chrome until this tool is migrated to the declarative controls.
  document.getElementById("tool-reset")?.remove();
  document.getElementById("tool-copy-output")?.remove();
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "960px";
  }

  const inputsContainer = document.querySelector(".tool-inputs");
  if (inputsContainer) {
    inputsContainer.style.display = "none";
  }

  out.innerHTML = "";
  out.className = "kd-converter-container";

  const grid = document.createElement("div");
  grid.className = "kd-grid";

  const leftPane = document.createElement("div");
  leftPane.className = "kd-pane";
  
  const leftLabel = document.createElement("label");
  leftLabel.className = "kd-pane-label";
  leftLabel.textContent = "Kruti Dev 010 (Remington Layout)";
  leftPane.appendChild(leftLabel);

  const leftTextarea = document.createElement("textarea");
  leftTextarea.className = "kd-textarea";
  leftTextarea.placeholder = "Type or paste legacy Kruti Dev text here...";
  leftTextarea.id = "kd-input-area";
  leftPane.appendChild(leftTextarea);

  const leftActions = document.createElement("div");
  leftActions.className = "kd-pane-actions";

  const leftCopyBtn = document.createElement("button");
  leftCopyBtn.className = "kd-btn kd-btn-copy";
  leftCopyBtn.innerHTML = `
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
      <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1z"/>
      <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0z"/>
    </svg> Copy
  `;
  leftActions.appendChild(leftCopyBtn);

  const leftClearBtn = document.createElement("button");
  leftClearBtn.className = "kd-btn kd-btn-clear";
  leftClearBtn.innerHTML = "Clear";
  leftActions.appendChild(leftClearBtn);

  leftPane.appendChild(leftActions);
  grid.appendChild(leftPane);

  const swapIndicator = document.createElement("div");
  swapIndicator.className = "kd-swap-indicator";
  swapIndicator.innerHTML = `
    <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" fill="currentColor" viewBox="0 0 16 16">
      <path fill-rule="evenodd" d="M1 11.5a.5.5 0 0 0 .5.5h11.793l-3.147 3.146a.5.5 0 0 0 .708.708l4-4a.5.5 0 0 0 0-.708l-4-4a.5.5 0 0 0-.708.708L13.293 11H1.5a.5.5 0 0 0-.5.5m14-7a.5.5 0 0 1-.5.5H2.707l3.147 3.146a.5.5 0 1 1-.708.708l-4-4a.5.5 0 0 1 0-.708l4-4a.5.5 0 1 1 .708.708L2.707 4H14.5a.5.5 0 0 1 .5.5"/>
    </svg>
  `;
  grid.appendChild(swapIndicator);

  const rightPane = document.createElement("div");
  rightPane.className = "kd-pane";

  const rightLabel = document.createElement("label");
  rightLabel.className = "kd-pane-label";
  rightLabel.textContent = "Unicode Devanagari (Standard Hindi)";
  rightPane.appendChild(rightLabel);

  const rightTextarea = document.createElement("textarea");
  rightTextarea.className = "kd-textarea";
  rightTextarea.placeholder = "Type or paste standard Unicode Hindi here...";
  rightTextarea.id = "kd-output-area";
  rightPane.appendChild(rightTextarea);

  const rightActions = document.createElement("div");
  rightActions.className = "kd-pane-actions";

  const rightCopyBtn = document.createElement("button");
  rightCopyBtn.className = "kd-btn kd-btn-copy";
  rightCopyBtn.innerHTML = `
    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
      <path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1z"/>
      <path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0z"/>
    </svg> Copy
  `;
  rightActions.appendChild(rightCopyBtn);

  const rightClearBtn = document.createElement("button");
  rightClearBtn.className = "kd-btn kd-btn-clear";
  rightClearBtn.innerHTML = "Clear";
  rightActions.appendChild(rightClearBtn);

  rightPane.appendChild(rightActions);
  grid.appendChild(rightPane);

  out.appendChild(grid);

  leftTextarea.addEventListener("input", () => {
    const val = leftTextarea.value;
    if (val.trim() === "") {
      rightTextarea.value = "";
      return;
    }
    try {
      const res = mod.krutidev_to_unicode(val);
      rightTextarea.value = res;
    } catch (e) {
      rightTextarea.value = "Error during conversion.";
    }
  });

  rightTextarea.addEventListener("input", () => {
    const val = rightTextarea.value;
    if (val.trim() === "") {
      leftTextarea.value = "";
      return;
    }
    try {
      const res = mod.unicode_to_krutidev(val);
      leftTextarea.value = res;
    } catch (e) {
      leftTextarea.value = "Error during conversion.";
    }
  });

  const setupCopy = (btn, textarea) => {
    btn.addEventListener("click", async () => {
      if (textarea.value.trim() === "") return;
      try {
        await navigator.clipboard.writeText(textarea.value);
        const originalHTML = btn.innerHTML;
        btn.innerHTML = `
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" fill="currentColor" viewBox="0 0 16 16">
            <path d="M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z"/>
          </svg> Copied!
        `;
        btn.classList.add("copied");
        setTimeout(() => {
          btn.innerHTML = originalHTML;
          btn.classList.remove("copied");
        }, 1500);
      } catch (err) {
        // Ignore
      }
    });
  };

  setupCopy(leftCopyBtn, leftTextarea);
  setupCopy(rightCopyBtn, rightTextarea);

  leftClearBtn.addEventListener("click", () => {
    leftTextarea.value = "";
    rightTextarea.value = "";
  });

  rightClearBtn.addEventListener("click", () => {
    leftTextarea.value = "";
    rightTextarea.value = "";
  });

  const params = new URLSearchParams(location.search);
  const prefillInput = params.get("input");
  if (prefillInput) {
    const isUni = prefillInput.split("").some((c) => {
      const u = c.charCodeAt(0);
      return u >= 0x0900 && u <= 0x097f;
    });
    if (isUni) {
      rightTextarea.value = prefillInput;
      rightTextarea.dispatchEvent(new Event("input"));
    } else {
      leftTextarea.value = prefillInput;
      leftTextarea.dispatchEvent(new Event("input"));
    }
  }
}

function renderListConverter(mod) {
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "960px";
  }

  // Get references to all 10 template-generated input elements.
  const inInput = document.getElementById("in-input");
  const inSep = document.getElementById("in-input_separator");
  const customInSep = document.getElementById("in-custom_input_separator");
  const outFormat = document.getElementById("in-output_format");
  const customOutSep = document.getElementById("in-custom_output_separator");
  const sortMode = document.getElementById("in-sort_mode");
  const dedupe = document.getElementById("in-dedupe");
  const caseTransform = document.getElementById("in-case_transform");
  const prefix = document.getElementById("in-prefix");
  const suffix = document.getElementById("in-suffix");

  // Keep references to inputs and detach them from their current parent.
  const inputs = [inInput, inSep, customInSep, outFormat, customOutSep, sortMode, dedupe, caseTransform, prefix, suffix];
  inputs.forEach(el => {
    if (el && el.parentNode) {
      el.parentNode.removeChild(el);
    }
  });

  // Clear non-output children of .tool-widget.
  if (widget) {
    const children = Array.from(widget.children);
    children.forEach(child => {
      if (child !== out) {
        widget.removeChild(child);
      }
    });
  }

  // Clear #tool-output and prepare it to host the dual-pane grid.
  out.innerHTML = "";
  out.className = "list-conv-container";

  const grid = document.createElement("div");
  grid.className = "list-conv-grid";

  // Left column (Inputs & Settings)
  const leftCol = document.createElement("div");
  leftCol.className = "list-conv-col-left";

  const inputLabel = document.createElement("label");
  inputLabel.className = "list-conv-label";
  inputLabel.textContent = "Input List";
  leftCol.appendChild(inputLabel);

  if (inInput) {
    inInput.className = "list-conv-textarea";
    inInput.placeholder = "Paste your list here (e.g. apple, banana, cherry)";
    inInput.rows = 10;
    leftCol.appendChild(inInput);
  }

  const settingsPanel = document.createElement("div");
  settingsPanel.className = "list-conv-settings-grid";

  // Split Separator
  const inSepGroup = document.createElement("div");
  inSepGroup.className = "list-conv-setting-group";
  const inSepLabel = document.createElement("label");
  inSepLabel.textContent = "Split Separator";
  inSepGroup.appendChild(inSepLabel);
  if (inSep) {
    inSep.className = "tool-select list-conv-select";
    inSep.innerHTML = "";
    const inSepOptions = [
      { value: "auto", text: "Auto-detect" },
      { value: "comma", text: "Comma (,)" },
      { value: "newline", text: "Newline" },
      { value: "semicolon", text: "Semicolon (;)" },
      { value: "space", text: "Space" },
      { value: "tab", text: "Tab" },
      { value: "pipe", text: "Pipe (|)" },
      { value: "custom", text: "Custom Delimiter" }
    ];
    inSepOptions.forEach(opt => {
      const o = document.createElement("option");
      o.value = opt.value;
      o.textContent = opt.text;
      inSep.appendChild(o);
    });
    inSepGroup.appendChild(inSep);
  }
  settingsPanel.appendChild(inSepGroup);

  // Custom Split Delimiter
  const customInSepGroup = document.createElement("div");
  customInSepGroup.className = "list-conv-setting-group list-conv-custom-in-group";
  customInSepGroup.style.display = "none";
  const customInSepLabel = document.createElement("label");
  customInSepLabel.textContent = "Custom Split Char";
  customInSepGroup.appendChild(customInSepLabel);
  if (customInSep) {
    customInSep.className = "tool-input list-conv-input";
    customInSep.placeholder = "e.g. ||";
    customInSepGroup.appendChild(customInSep);
  }
  settingsPanel.appendChild(customInSepGroup);

  // Output Layout
  const outFormatGroup = document.createElement("div");
  outFormatGroup.className = "list-conv-setting-group";
  const outFormatLabel = document.createElement("label");
  outFormatLabel.textContent = "Output Layout";
  outFormatGroup.appendChild(outFormatLabel);
  if (outFormat) {
    outFormat.className = "tool-select list-conv-select";
    outFormat.innerHTML = "";
    const outFormatOptions = [
      { value: "newline", text: "Newline (One per line)" },
      { value: "comma", text: "Comma-separated (,)" },
      { value: "bulleted", text: "Bulleted List (-)" },
      { value: "numbered", text: "Numbered List (1.)" },
      { value: "quoted", text: "Double Quoted (\"...\")" },
      { value: "space", text: "Space-separated" },
      { value: "tab", text: "Tab-separated" },
      { value: "pipe", text: "Pipe-separated (|)" },
      { value: "json", text: "JSON Array [...]" },
      { value: "xml", text: "XML Elements (<item>)" },
      { value: "sql", text: "SQL IN Clause ('...')" },
      { value: "custom", text: "Custom Delimiter" }
    ];
    outFormatOptions.forEach(opt => {
      const o = document.createElement("option");
      o.value = opt.value;
      o.textContent = opt.text;
      outFormat.appendChild(o);
    });
    outFormatGroup.appendChild(outFormat);
  }
  settingsPanel.appendChild(outFormatGroup);

  // Custom Join Delimiter
  const customOutSepGroup = document.createElement("div");
  customOutSepGroup.className = "list-conv-setting-group list-conv-custom-out-group";
  customOutSepGroup.style.display = "none";
  const customOutSepLabel = document.createElement("label");
  customOutSepLabel.textContent = "Custom Join Char";
  customOutSepGroup.appendChild(customOutSepLabel);
  if (customOutSep) {
    customOutSep.className = "tool-input list-conv-input";
    customOutSep.placeholder = "e.g. #";
    customOutSepGroup.appendChild(customOutSep);
  }
  settingsPanel.appendChild(customOutSepGroup);

  // Sort Order
  const sortModeGroup = document.createElement("div");
  sortModeGroup.className = "list-conv-setting-group";
  const sortModeLabel = document.createElement("label");
  sortModeLabel.textContent = "Sort Order";
  sortModeGroup.appendChild(sortModeLabel);
  if (sortMode) {
    sortMode.className = "tool-select list-conv-select";
    sortMode.innerHTML = "";
    const sortModeOptions = [
      { value: "none", text: "No Sorting" },
      { value: "asc", text: "Alphabetical (A-Z)" },
      { value: "desc", text: "Alphabetical (Z-A)" },
      { value: "length_asc", text: "Length (Shortest first)" },
      { value: "length_desc", text: "Length (Longest first)" },
      { value: "shuffle", text: "Randomize / Shuffle" }
    ];
    sortModeOptions.forEach(opt => {
      const o = document.createElement("option");
      o.value = opt.value;
      o.textContent = opt.text;
      sortMode.appendChild(o);
    });
    sortModeGroup.appendChild(sortMode);
  }
  settingsPanel.appendChild(sortModeGroup);

  // Text Case
  const caseGroup = document.createElement("div");
  caseGroup.className = "list-conv-setting-group";
  const caseLabel = document.createElement("label");
  caseLabel.textContent = "Text Case";
  caseGroup.appendChild(caseLabel);
  if (caseTransform) {
    caseTransform.className = "tool-select list-conv-select";
    caseTransform.innerHTML = "";
    const caseOptions = [
      { value: "none", text: "No Case Conversion" },
      { value: "lowercase", text: "Lowercase" },
      { value: "uppercase", text: "Uppercase" },
      { value: "titlecase", text: "Title Case" }
    ];
    caseOptions.forEach(opt => {
      const o = document.createElement("option");
      o.value = opt.value;
      o.textContent = opt.text;
      caseTransform.appendChild(o);
    });
    caseGroup.appendChild(caseTransform);
  }
  settingsPanel.appendChild(caseGroup);

  // Prefix
  const prefixGroup = document.createElement("div");
  prefixGroup.className = "list-conv-setting-group";
  const prefixLabel = document.createElement("label");
  prefixLabel.textContent = "Add Prefix";
  prefixGroup.appendChild(prefixLabel);
  if (prefix) {
    prefix.className = "tool-input list-conv-input";
    prefix.placeholder = "e.g. id_";
    prefixGroup.appendChild(prefix);
  }
  settingsPanel.appendChild(prefixGroup);

  // Suffix
  const suffixGroup = document.createElement("div");
  suffixGroup.className = "list-conv-setting-group";
  const suffixLabel = document.createElement("label");
  suffixLabel.textContent = "Add Suffix";
  suffixGroup.appendChild(suffixLabel);
  if (suffix) {
    suffix.className = "tool-input list-conv-input";
    suffix.placeholder = "e.g. _v1";
    suffixGroup.appendChild(suffix);
  }
  settingsPanel.appendChild(suffixGroup);

  // Deduplicate
  const checkGroup = document.createElement("div");
  checkGroup.className = "list-conv-check-group";
  const dedupeLabel = document.createElement("label");
  dedupeLabel.className = "list-conv-checkbox-label";
  if (dedupe) {
    dedupe.className = "list-conv-checkbox";
    dedupeLabel.appendChild(dedupe);
  }
  dedupeLabel.appendChild(document.createTextNode(" Remove Duplicates"));
  checkGroup.appendChild(dedupeLabel);
  settingsPanel.appendChild(checkGroup);

  leftCol.appendChild(settingsPanel);
  grid.appendChild(leftCol);

  // Right Column (Output & Actions)
  const rightCol = document.createElement("div");
  rightCol.className = "list-conv-col-right";

  const outputLabel = document.createElement("label");
  outputLabel.className = "list-conv-label";
  outputLabel.textContent = "Formatted List";
  rightCol.appendChild(outputLabel);

  const outputTextarea = document.createElement("textarea");
  outputTextarea.className = "list-conv-textarea";
  outputTextarea.readOnly = true;
  outputTextarea.placeholder = "Formatted output will appear here...";
  outputTextarea.id = "list-conv-output-area";
  rightCol.appendChild(outputTextarea);

  // Stats Card
  const statsCard = document.createElement("div");
  statsCard.className = "list-conv-stats-card";
  statsCard.innerHTML = `
    <div class="list-conv-stat"><span id="list-conv-stat-in">0</span> items in</div>
    <div class="list-conv-stat-arrow">➔</div>
    <div class="list-conv-stat"><span id="list-conv-stat-out">0</span> items out</div>
    <div class="list-conv-stat-diff" id="list-conv-stat-diff-container" style="display:none;">
      (<span id="list-conv-stat-diff">0</span> duplicates removed)
    </div>
  `;
  rightCol.appendChild(statsCard);

  // Actions Row
  const actionsRow = document.createElement("div");
  actionsRow.className = "list-conv-actions";

  const swapBtn = document.createElement("button");
  swapBtn.type = "button";
  swapBtn.className = "list-conv-btn list-conv-btn-swap";
  swapBtn.innerHTML = `
    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" viewBox="0 0 24 24"><path d="M17 3L21 7L17 11"/><path d="M3 14H17"/><path d="M7 21L3 17L7 13"/><path d="M21 10H7"/></svg>
    Swap Output
  `;
  actionsRow.appendChild(swapBtn);

  const resetBtn = document.createElement("button");
  resetBtn.type = "button";
  resetBtn.className = "list-conv-btn list-conv-btn-clear";
  resetBtn.textContent = "Reset";
  actionsRow.appendChild(resetBtn);

  const copyBtn = document.createElement("button");
  copyBtn.type = "button";
  copyBtn.className = "list-conv-btn list-conv-btn-copy";
  copyBtn.innerHTML = `
    <svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg>
    <svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="display:none;"><polyline points="20 6 9 17 4 12"></polyline></svg>
    Copy Result
  `;
  actionsRow.appendChild(copyBtn);

  rightCol.appendChild(actionsRow);
  grid.appendChild(rightCol);
  out.appendChild(grid);

  // Setup display toggles
  if (inSep) {
    inSep.addEventListener("change", () => {
      customInSepGroup.style.display = inSep.value === "custom" ? "flex" : "none";
    });
  }
  if (outFormat) {
    outFormat.addEventListener("change", () => {
      customOutSepGroup.style.display = outFormat.value === "custom" ? "flex" : "none";
    });
  }

  // Reset button logic
  resetBtn.addEventListener("click", () => {
    if (inInput) inInput.value = "";
    if (inSep) inSep.value = "auto";
    if (customInSep) customInSep.value = "";
    if (outFormat) outFormat.value = "newline";
    if (customOutSep) customOutSep.value = "";
    if (sortMode) sortMode.value = "none";
    if (dedupe) dedupe.checked = false;
    if (caseTransform) caseTransform.value = "none";
    if (prefix) prefix.value = "";
    if (suffix) suffix.value = "";

    customInSepGroup.style.display = "none";
    customOutSepGroup.style.display = "none";

    if (inInput) {
      inInput.dispatchEvent(new Event("input", { bubbles: true }));
    }
  });

  // Swap button logic
  swapBtn.addEventListener("click", () => {
    const outVal = outputTextarea.value;
    if (!outVal.trim() || !inInput || !inSep || !outFormat) return;

    inInput.value = outVal;
    const inSepVal = inSep.value;
    const outFmtVal = outFormat.value;

    let newInSep = "auto";
    if (outFmtVal === "comma" || outFmtVal === "quoted" || outFmtVal === "json" || outFmtVal === "sql") newInSep = "comma";
    else if (outFmtVal === "newline" || outFmtVal === "bulleted" || outFmtVal === "numbered" || outFmtVal === "xml") newInSep = "newline";
    else if (outFmtVal === "space") newInSep = "space";
    else if (outFmtVal === "tab") newInSep = "tab";
    else if (outFmtVal === "pipe") newInSep = "pipe";
    else if (outFmtVal === "custom") {
      newInSep = "custom";
      if (customInSep && customOutSep) {
        customInSep.value = customOutSep.value;
      }
    }
    inSep.value = newInSep;

    let newOutFmt = "newline";
    if (inSepVal === "comma") newOutFmt = "comma";
    else if (inSepVal === "newline") newOutFmt = "newline";
    else if (inSepVal === "space") newOutFmt = "space";
    else if (inSepVal === "tab") newOutFmt = "tab";
    else if (inSepVal === "pipe") newOutFmt = "pipe";
    else if (inSepVal === "custom") {
      newOutFmt = "custom";
      if (customOutSep && customInSep) {
        customOutSep.value = customInSep.value;
      }
    }
    outFormat.value = newOutFmt;

    customInSepGroup.style.display = inSep.value === "custom" ? "flex" : "none";
    customOutSepGroup.style.display = outFormat.value === "custom" ? "flex" : "none";

    inInput.dispatchEvent(new Event("input", { bubbles: true }));
  });

  // Copy button logic
  copyBtn.addEventListener("click", async () => {
    const text = outputTextarea.value;
    if (!text.trim()) return;

    try {
      await navigator.clipboard.writeText(text);
      const copyIcon = copyBtn.querySelector(".copy-icon");
      const checkIcon = copyBtn.querySelector(".check-icon");

      copyIcon.style.display = "none";
      checkIcon.style.display = "inline-block";
      copyBtn.classList.add("copied");

      setTimeout(() => {
        copyIcon.style.display = "inline-block";
        checkIcon.style.display = "none";
        copyBtn.classList.remove("copied");
      }, 1500);
    } catch (err) {
      // ignore
    }
  });
}

function renderListConverterResult(value) {
  const outputTextarea = document.getElementById("list-conv-output-area");
  if (outputTextarea) {
    outputTextarea.value = value;
  }

  const inInput = document.getElementById("in-input");
  const inSep = document.getElementById("in-input_separator");
  const customInSep = document.getElementById("in-custom_input_separator");
  const outFormat = document.getElementById("in-output_format");
  const customOutSep = document.getElementById("in-custom_output_separator");
  const dedupe = document.getElementById("in-dedupe");

  const customInSepGroup = document.querySelector(".list-conv-custom-in-group");
  const customOutSepGroup = document.querySelector(".list-conv-custom-out-group");

  if (inSep && customInSepGroup) {
    customInSepGroup.style.display = inSep.value === "custom" ? "flex" : "none";
  }
  if (outFormat && customOutSepGroup) {
    customOutSepGroup.style.display = outFormat.value === "custom" ? "flex" : "none";
  }

  if (!inInput) return;

  const text = inInput.value;
  if (!text.trim()) {
    const statIn = document.getElementById("list-conv-stat-in");
    const statOut = document.getElementById("list-conv-stat-out");
    const diffContainer = document.getElementById("list-conv-stat-diff-container");
    if (statIn) statIn.textContent = "0";
    if (statOut) statOut.textContent = "0";
    if (diffContainer) diffContainer.style.display = "none";
    return;
  }

  const inCount = countListItems(text, inSep ? inSep.value : "auto", customInSep ? customInSep.value : "");
  let outCount = inCount;

  if (dedupe && dedupe.checked) {
    const fmt = outFormat ? outFormat.value : "newline";
    let outSep = fmt;
    if (fmt === "custom") {
      outSep = "custom";
    } else if (fmt === "quoted" || fmt === "json" || fmt === "sql" || fmt === "comma") {
      outSep = "comma";
    } else if (fmt === "bulleted" || fmt === "numbered" || fmt === "xml") {
      outSep = "newline";
    }
    outCount = countListItems(value, outSep, customOutSep ? customOutSep.value : "");
  }

  const statIn = document.getElementById("list-conv-stat-in");
  const statOut = document.getElementById("list-conv-stat-out");
  if (statIn) statIn.textContent = inCount;
  if (statOut) statOut.textContent = outCount;

  const diff = inCount - outCount;
  const diffContainer = document.getElementById("list-conv-stat-diff-container");
  if (diff > 0 && diffContainer) {
    diffContainer.style.display = "inline";
    const diffEl = document.getElementById("list-conv-stat-diff");
    if (diffEl) diffEl.textContent = diff;
  } else if (diffContainer) {
    diffContainer.style.display = "none";
  }
}

function countListItems(text, sepType, customSep) {
  if (!text.trim()) return 0;
  let sep = sepType;
  if (sep === "auto") {
    if (text.includes("\n")) sep = "newline";
    else if (text.includes(",")) sep = "comma";
    else if (text.includes(";")) sep = "semicolon";
    else if (text.includes("|")) sep = "pipe";
    else if (text.includes("\t")) sep = "tab";
    else return 1;
  }
  let parts = [];
  if (sep === "comma") parts = text.split(",");
  else if (sep === "newline") parts = text.split("\n");
  else if (sep === "semicolon") parts = text.split(";");
  else if (sep === "space") parts = text.split(/\s+/);
  else if (sep === "tab") parts = text.split("\t");
  else if (sep === "pipe") parts = text.split("|");
  else if (sep === "custom") {
    if (!customSep) return text.split("\n").length;
    parts = text.split(customSep);
  }
  return parts.map(s => s.trim()).filter(s => s.length > 0).length;
}

function setupJwtDecodeDefaults() {
  // Legacy custom dashboard output — the generic Copy-result would copy its
  // concatenated labels; drop it until this tool is migrated. Reset stays (the
  // token field is a normal input).
  document.getElementById("tool-copy-output")?.remove();
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "880px";
  }
}

function renderJwtDecode(data) {
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "880px";
  }

  out.innerHTML = "";
  out.className = "jwt-decode-container";

  const grid = document.createElement("div");
  grid.className = "jwt-grid";

  // Left Column: Encoded Token Colorized View
  const leftCol = document.createElement("div");
  leftCol.className = "jwt-col-left";

  const visualTitle = document.createElement("h3");
  visualTitle.className = "jwt-section-title";
  visualTitle.textContent = "Encoded Token";
  leftCol.appendChild(visualTitle);

  const highlightBox = document.createElement("div");
  highlightBox.className = "jwt-highlight-box";

  const tokenInput = document.getElementById("in-token");
  const tokenStr = tokenInput ? tokenInput.value.trim() : "";
  const parts = tokenStr.split(".");

  if (parts.length >= 1 && parts[0]) {
    const spanHeader = document.createElement("span");
    spanHeader.className = "jwt-token-part jwt-color-header";
    spanHeader.textContent = parts[0];
    highlightBox.appendChild(spanHeader);
  }
  if (parts.length >= 2) {
    const dot1 = document.createTextNode(".");
    highlightBox.appendChild(dot1);
    const spanPayload = document.createElement("span");
    spanPayload.className = "jwt-token-part jwt-color-payload";
    spanPayload.textContent = parts[1];
    highlightBox.appendChild(spanPayload);
  }
  if (parts.length >= 3) {
    const dot2 = document.createTextNode(".");
    highlightBox.appendChild(dot2);
    const spanSignature = document.createElement("span");
    spanSignature.className = "jwt-token-part jwt-color-signature";
    spanSignature.textContent = parts[2];
    highlightBox.appendChild(spanSignature);
  }
  leftCol.appendChild(highlightBox);
  grid.appendChild(leftCol);

  // Right Column: Claims Validation & Decoded JSON
  const rightCol = document.createElement("div");
  rightCol.className = "jwt-col-right";

  const statusBanner = document.createElement("div");
  statusBanner.className = `jwt-status-banner ${data.valid ? "jwt-status-valid" : "jwt-status-invalid"}`;

  const statusText = document.createElement("div");
  statusText.className = "jwt-status-text";
  statusText.innerHTML = data.valid
    ? `<strong>Active Token</strong> — Standard time claims are currently valid.`
    : `<strong>Validation Flagged</strong> — ${data.error || "Token has expired or is not yet valid."}`;
  statusBanner.appendChild(statusText);
  rightCol.appendChild(statusBanner);

  // Claims Validation Card
  const checksCard = document.createElement("div");
  checksCard.className = "jwt-card jwt-checks-card";

  const checksTitle = document.createElement("h4");
  checksTitle.className = "jwt-card-title";
  checksTitle.textContent = "Claims Validation";
  checksCard.appendChild(checksTitle);

  const checksList = document.createElement("div");
  checksList.className = "jwt-checks-list";

  data.checks.forEach((c) => {
    const checkItem = document.createElement("div");
    checkItem.className = `jwt-check-item ${c.ok ? "jwt-check-ok" : "jwt-check-fail"}`;

    const icon = document.createElement("span");
    icon.className = "jwt-check-icon";
    icon.innerHTML = c.ok
      ? `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`
      : `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>`;
    checkItem.appendChild(icon);

    const info = document.createElement("div");
    info.className = "jwt-check-info";

    const name = document.createElement("div");
    name.className = "jwt-check-name";
    name.textContent = c.name === "signature_present" ? "Signature segment" : c.name.toUpperCase();
    info.appendChild(name);

    const desc = document.createElement("div");
    desc.className = "jwt-check-desc";
    desc.textContent = c.detail;
    info.appendChild(desc);

    checkItem.appendChild(info);
    checksList.appendChild(checkItem);
  });
  checksCard.appendChild(checksList);
  rightCol.appendChild(checksCard);

  // Decoded Header Card
  const headerCard = document.createElement("div");
  headerCard.className = "jwt-card jwt-decoded-card jwt-decoded-header-card";

  const headerTitleWrap = document.createElement("div");
  headerTitleWrap.className = "jwt-card-title-wrap";

  const headerTitle = document.createElement("h4");
  headerTitle.className = "jwt-card-title jwt-color-header-text";
  headerTitle.textContent = "HEADER: ALGORITHM & TOKEN TYPE";
  headerTitleWrap.appendChild(headerTitle);

  const headerCopy = document.createElement("button");
  headerCopy.className = "jwt-copy-btn";
  headerCopy.title = "Copy Header JSON";
  const headerJsonStr = JSON.stringify(data.header, null, 2);
  headerCopy.addEventListener("click", () => {
    navigator.clipboard.writeText(headerJsonStr).then(() => {
      headerCopy.classList.add("copied");
      setTimeout(() => headerCopy.classList.remove("copied"), 2000);
    });
  });
  headerCopy.innerHTML = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg><svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
  headerTitleWrap.appendChild(headerCopy);
  headerCard.appendChild(headerTitleWrap);

  const headerCode = document.createElement("pre");
  headerCode.className = "jwt-code-block";
  headerCode.textContent = headerJsonStr;
  headerCard.appendChild(headerCode);
  rightCol.appendChild(headerCard);

  // Decoded Payload Card
  const payloadCard = document.createElement("div");
  payloadCard.className = "jwt-card jwt-decoded-card jwt-decoded-payload-card";

  const payloadTitleWrap = document.createElement("div");
  payloadTitleWrap.className = "jwt-card-title-wrap";

  const payloadTitle = document.createElement("h4");
  payloadTitle.className = "jwt-card-title jwt-color-payload-text";
  payloadTitle.textContent = "PAYLOAD: DATA / CLAIMS";
  payloadTitleWrap.appendChild(payloadTitle);

  const payloadCopy = document.createElement("button");
  payloadCopy.className = "jwt-copy-btn";
  payloadCopy.title = "Copy Payload JSON";
  const payloadJsonStr = JSON.stringify(data.payload, null, 2);
  payloadCopy.addEventListener("click", () => {
    navigator.clipboard.writeText(payloadJsonStr).then(() => {
      payloadCopy.classList.add("copied");
      setTimeout(() => payloadCopy.classList.remove("copied"), 2000);
    });
  });
  payloadCopy.innerHTML = `<svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg><svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
  payloadTitleWrap.appendChild(payloadCopy);
  payloadCard.appendChild(payloadTitleWrap);

  const payloadCode = document.createElement("pre");
  payloadCode.className = "jwt-code-block";
  payloadCode.textContent = payloadJsonStr;
  payloadCard.appendChild(payloadCode);
  rightCol.appendChild(payloadCard);

  grid.appendChild(rightCol);
  out.appendChild(grid);
}

function setupImageCropCustomUI() {
  const widget = document.querySelector(".tool-widget");
  const fileInput = document.getElementById("in-image");
  const xInput = document.getElementById("in-x");
  const yInput = document.getElementById("in-y");
  const widthInput = document.getElementById("in-width");
  const heightInput = document.getElementById("in-height");
  const media = document.getElementById("tool-output-media");
  const dl = document.getElementById("tool-output-download");
  const statusOut = document.getElementById("tool-output");

  if (!widget || !fileInput || !xInput || !yInput || !widthInput || !heightInput) return;

  // 1. Hide native file input
  fileInput.style.display = "none";

  // 2. Add premium drag-and-drop zone
  const dropZone = document.createElement("div");
  dropZone.className = "crop-dropzone";
  dropZone.innerHTML = `
    <svg class="crop-dropzone-icon" viewBox="0 0 24 24" width="32" height="32" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
    <div class="crop-dropzone-text">Drag & drop your image here, or <span>browse</span></div>
  `;
  fileInput.parentNode.insertBefore(dropZone, fileInput);

  // Click triggers file selector
  dropZone.addEventListener("click", () => fileInput.click());

  // Drag and drop event handling
  dropZone.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropZone.classList.add("dragover");
  });
  dropZone.addEventListener("dragleave", () => {
    dropZone.classList.remove("dragover");
  });
  dropZone.addEventListener("drop", (e) => {
    e.preventDefault();
    dropZone.classList.remove("dragover");
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      fileInput.files = e.dataTransfer.files;
      fileInput.dispatchEvent(new Event("change"));
    }
  });

  // Full-screen window drag and drop support
  const fullscreenOverlay = document.createElement("div");
  fullscreenOverlay.className = "crop-fullscreen-drag-overlay";
  fullscreenOverlay.innerHTML = `
    <svg viewBox="0 0 24 24" width="48" height="48" stroke="currentColor" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M17 8l-5-5-5 5M12 3v12"/></svg>
    <div class="crop-fullscreen-drag-overlay-text">Drop image to crop</div>
  `;
  document.body.appendChild(fullscreenOverlay);

  let dragCounter = 0;
  window.addEventListener("dragenter", (e) => {
    e.preventDefault();
    dragCounter++;
    if (dragCounter === 1) {
      fullscreenOverlay.classList.add("active");
    }
  });

  window.addEventListener("dragover", (e) => {
    e.preventDefault();
  });

  window.addEventListener("dragleave", (e) => {
    e.preventDefault();
    dragCounter--;
    if (dragCounter === 0) {
      fullscreenOverlay.classList.remove("active");
    }
  });

  window.addEventListener("drop", (e) => {
    e.preventDefault();
    dragCounter = 0;
    fullscreenOverlay.classList.remove("active");
    if (e.dataTransfer.files && e.dataTransfer.files[0]) {
      const file = e.dataTransfer.files[0];
      if (file.type.startsWith("image/")) {
        fileInput.files = e.dataTransfer.files;
        fileInput.dispatchEvent(new Event("change"));
      }
    }
  });

  // 3. Build side-by-side workspace
  const workspace = document.createElement("div");
  workspace.className = "crop-workspace";
  workspace.style.display = "none"; // hidden until file loaded

  const leftCol = document.createElement("div");
  leftCol.className = "crop-workspace-left";

  const rightCol = document.createElement("div");
  rightCol.className = "crop-workspace-right";

  workspace.appendChild(leftCol);
  workspace.appendChild(rightCol);
  widget.appendChild(workspace);

  // Group inputs into right column grid
  const coordsGrid = document.createElement("div");
  coordsGrid.className = "crop-coords-grid";

  const fields = [
    { id: "in-x", label: "X Offset" },
    { id: "in-y", label: "Y Offset" },
    { id: "in-width", label: "Width" },
    { id: "in-height", label: "Height" }
  ];

  fields.forEach(field => {
    const col = document.createElement("div");
    col.className = "crop-coords-col";
    
    const el = document.getElementById(field.id);
    const labelEl = document.querySelector(`label[for="${field.id}"]`);
    
    if (labelEl) {
      labelEl.textContent = field.label;
      col.appendChild(labelEl);
    }
    if (el) {
      el.type = "number";
      el.min = "0";
      col.appendChild(el);
    }
    coordsGrid.appendChild(col);
  });

  rightCol.appendChild(coordsGrid);

  // Add Reset Button
  const resetBtn = document.createElement("button");
  resetBtn.type = "button";
  resetBtn.className = "crop-reset-btn";
  resetBtn.innerHTML = `
    <svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg>
    Reset Crop
  `;
  rightCol.appendChild(resetBtn);

  // Move outputs to right column
  const outputLabel = widget.querySelector(".tool-output-label");
  if (outputLabel) rightCol.appendChild(outputLabel);
  if (media) rightCol.appendChild(media);
  if (dl) {
    dl.innerHTML = `<svg viewBox="0 0 24 24" width="16" height="16" stroke="currentColor" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 4px;"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg> Download Crop`;
    rightCol.appendChild(dl);
  }
  if (statusOut) rightCol.appendChild(statusOut);

  // 4. Set up interactive left column preview and overlay
  const previewContainer = document.createElement("div");
  previewContainer.className = "crop-preview-container";

  const previewImg = document.createElement("img");
  previewImg.className = "crop-preview-img";
  previewImg.id = "crop-preview-img";
  previewContainer.appendChild(previewImg);

  const overlayRect = document.createElement("div");
  overlayRect.className = "crop-overlay-rect";
  overlayRect.id = "crop-overlay-rect";

  // Corners
  const handles = ["tl", "tr", "bl", "br"];
  handles.forEach(h => {
    const handle = document.createElement("div");
    handle.className = `crop-handle crop-handle-${h}`;
    handle.dataset.handle = h;
    overlayRect.appendChild(handle);
  });

  previewContainer.appendChild(overlayRect);
  leftCol.appendChild(previewContainer);

  // Bind image reader
  fileInput.addEventListener("change", () => {
    const file = fileInput.files && fileInput.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      previewImg.src = e.target.result;
    };
    reader.readAsDataURL(file);
  });

  previewImg.onload = () => {
    widget.style.maxWidth = "840px";
    dropZone.style.display = "none";
    workspace.style.display = "flex";

    const naturalW = previewImg.naturalWidth;
    const naturalH = previewImg.naturalHeight;

    xInput.max = naturalW - 1;
    yInput.max = naturalH - 1;
    widthInput.max = naturalW;
    heightInput.max = naturalH;

    const params = new URLSearchParams(location.search);
    const hasParams = params.has("x") || params.has("y") || params.has("width") || params.has("height");

    if (!hasParams || !xInput.value || !yInput.value || !widthInput.value || !heightInput.value) {
      // Smart Default: Centered 80% box
      const w = Math.round(naturalW * 0.8) || 1;
      const h = Math.round(naturalH * 0.8) || 1;
      const x = Math.round((naturalW - w) / 2) || 0;
      const y = Math.round((naturalH - h) / 2) || 0;

      xInput.value = x;
      yInput.value = y;
      widthInput.value = w;
      heightInput.value = h;
    }

    updateOverlayFromInputs();
    
    // Programmatically trigger ffmpeg execution after defaults are set and overlay is positioned
    widthInput.dispatchEvent(new Event("change"));
  };

  function updateOverlayFromInputs() {
    const naturalW = previewImg.naturalWidth;
    const naturalH = previewImg.naturalHeight;
    if (!naturalW || !naturalH) return;

    const clientW = previewImg.clientWidth;
    const clientH = previewImg.clientHeight;

    const scaleX = clientW / naturalW;
    const scaleY = clientH / naturalH;

    let x = parseFloat(xInput.value) || 0;
    let y = parseFloat(yInput.value) || 0;
    let w = parseFloat(widthInput.value) || naturalW;
    let h = parseFloat(heightInput.value) || naturalH;

    x = Math.max(0, Math.min(naturalW - 1, x));
    y = Math.max(0, Math.min(naturalH - 1, y));
    w = Math.max(1, Math.min(naturalW - x, w));
    h = Math.max(1, Math.min(naturalH - y, h));

    overlayRect.style.left = `${x * scaleX}px`;
    overlayRect.style.top = `${y * scaleY}px`;
    overlayRect.style.width = `${w * scaleX}px`;
    overlayRect.style.height = `${h * scaleY}px`;
  }

  [xInput, yInput, widthInput, heightInput].forEach(inp => {
    inp.addEventListener("input", updateOverlayFromInputs);
  });
  window.addEventListener("resize", updateOverlayFromInputs);

  // Dragging and resizing overlay interaction
  let isDragging = false;
  let activeHandle = null;
  let startX, startY;
  const startRect = { x: 0, y: 0, w: 0, h: 0 };

  overlayRect.addEventListener("mousedown", dragStart);
  overlayRect.addEventListener("touchstart", dragStart, { passive: false });

  function dragStart(e) {
    e.preventDefault();
    isDragging = true;

    const target = e.target;
    if (target.classList.contains("crop-handle")) {
      activeHandle = target.dataset.handle;
    } else {
      activeHandle = null; // dragging center
    }

    const clientX = e.touches ? e.touches[0].clientX : e.clientX;
    const clientY = e.touches ? e.touches[0].clientY : e.clientY;

    startX = clientX;
    startY = clientY;

    startRect.x = parseFloat(overlayRect.style.left) || 0;
    startRect.y = parseFloat(overlayRect.style.top) || 0;
    startRect.w = parseFloat(overlayRect.style.width) || 0;
    startRect.h = parseFloat(overlayRect.style.height) || 0;

    document.addEventListener("mousemove", dragMove);
    document.addEventListener("touchmove", dragMove, { passive: false });
    document.addEventListener("mouseup", dragEnd);
    document.addEventListener("touchend", dragEnd);
  }

  function dragMove(e) {
    if (!isDragging) return;
    e.preventDefault();

    const clientX = e.touches ? e.touches[0].clientX : e.clientX;
    const clientY = e.touches ? e.touches[0].clientY : e.clientY;

    const dx = clientX - startX;
    const dy = clientY - startY;

    const clientW = previewImg.clientWidth;
    const clientH = previewImg.clientHeight;

    let newX = startRect.x;
    let newY = startRect.y;
    let newW = startRect.w;
    let newH = startRect.h;

    if (activeHandle === null) {
      // Drag center
      newX = Math.max(0, Math.min(clientW - startRect.w, startRect.x + dx));
      newY = Math.max(0, Math.min(clientH - startRect.h, startRect.y + dy));
    } else {
      // Drag handles
      if (activeHandle.includes("l")) {
        const proposedX = startRect.x + dx;
        const proposedW = startRect.w - dx;
        if (proposedX >= 0 && proposedW >= 8) {
          newX = proposedX;
          newW = proposedW;
        }
      }
      if (activeHandle.includes("r")) {
        newW = Math.max(8, Math.min(clientW - startRect.x, startRect.w + dx));
      }
      if (activeHandle.includes("t")) {
        const proposedY = startRect.y + dy;
        const proposedH = startRect.h - dy;
        if (proposedY >= 0 && proposedH >= 8) {
          newY = proposedY;
          newH = proposedH;
        }
      }
      if (activeHandle.includes("b")) {
        newH = Math.max(8, Math.min(clientH - startRect.y, startRect.h + dy));
      }
    }

    overlayRect.style.left = `${newX}px`;
    overlayRect.style.top = `${newY}px`;
    overlayRect.style.width = `${newW}px`;
    overlayRect.style.height = `${newH}px`;

    const scaleX = previewImg.naturalWidth / clientW;
    const scaleY = previewImg.naturalHeight / clientH;

    xInput.value = Math.round(newX * scaleX);
    yInput.value = Math.round(newY * scaleY);
    widthInput.value = Math.round(newW * scaleX);
    heightInput.value = Math.round(newH * scaleY);
  }

  function dragEnd(e) {
    if (!isDragging) return;
    isDragging = false;

    document.removeEventListener("mousemove", dragMove);
    document.removeEventListener("touchmove", dragMove);
    document.removeEventListener("mouseup", dragEnd);
    document.removeEventListener("touchend", dragEnd);

    // Trigger compute on change
    widthInput.dispatchEvent(new Event("change"));
  }

  // Reset Button
  resetBtn.addEventListener("click", () => {
    const naturalW = previewImg.naturalWidth;
    const naturalH = previewImg.naturalHeight;
    if (!naturalW || !naturalH) return;

    const w = Math.round(naturalW * 0.8) || 1;
    const h = Math.round(naturalH * 0.8) || 1;
    const x = Math.round((naturalW - w) / 2) || 0;
    const y = Math.round((naturalH - h) / 2) || 0;

    xInput.value = x;
    yInput.value = y;
    widthInput.value = w;
    heightInput.value = h;

    updateOverlayFromInputs();
    widthInput.dispatchEvent(new Event("change"));
  });
}

main();

