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
  }
  out.textContent = cfg.format === "number" ? formatNumber(value) : String(value);
}

function showError(message) {
  const widget = document.querySelector(".tool-widget");
  if (widget && cfg.slug !== "timezone-convert" && cfg.slug !== "age-calculator") {
    widget.style.maxWidth = "380px";
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
  } else if (cfg.slug === "age-calculator") {
    setupAgeCalculatorDefaults();
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
        const widget = document.querySelector(".tool-widget");
        if (widget && cfg.slug !== "timezone-convert") {
          widget.style.maxWidth = "380px";
        }
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

function setupAgeCalculatorDefaults() {
  const widget = document.querySelector(".tool-widget");
  if (widget) {
    widget.style.maxWidth = "760px";
  }

  const birthInput = document.getElementById("in-birthdate");
  const asOfInput = document.getElementById("in-as_of");

  const birthLabel = document.querySelector('label[for="in-birthdate"]');
  const asOfLabel = document.querySelector('label[for="in-as_of"]');

  if (birthInput) {
    birthInput.type = "date";
  }
  if (asOfInput) {
    asOfInput.type = "date";
  }

  // Format current local date: YYYY-MM-DD
  const now = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  const localDateStr = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;

  const params = new URLSearchParams(location.search);
  if (asOfInput && !asOfInput.value && !params.has("as_of")) {
    asOfInput.value = localDateStr;
  }

  if (widget && birthInput && asOfInput) {
    const wrapper = document.createElement("div");
    wrapper.className = "age-inputs-wrapper";

    const col1 = document.createElement("div");
    col1.className = "age-inputs-col";
    if (birthLabel) col1.appendChild(birthLabel);
    col1.appendChild(birthInput);

    const col2 = document.createElement("div");
    col2.className = "age-inputs-col";
    if (asOfLabel) col2.appendChild(asOfLabel);
    col2.appendChild(asOfInput);

    const col3 = document.createElement("div");
    col3.className = "age-inputs-col";
    
    const resetBtn = document.createElement("button");
    resetBtn.type = "button";
    resetBtn.className = "age-reset-btn";
    resetBtn.innerHTML = `<svg viewBox="0 0 24 24" width="14" height="14" stroke="currentColor" stroke-width="2.5" fill="none" stroke-linecap="round" stroke-linejoin="round"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg> Reset`;
    col3.appendChild(resetBtn);

    wrapper.appendChild(col1);
    wrapper.appendChild(col2);
    wrapper.appendChild(col3);

    widget.insertBefore(wrapper, widget.firstChild);

    resetBtn.addEventListener("click", () => {
      birthInput.value = "";
      asOfInput.value = localDateStr;
      
      // Clear URL params on reset
      const newUrl = window.location.pathname;
      window.history.replaceState({}, document.title, newUrl);

      birthInput.dispatchEvent(new Event("input"));
      asOfInput.dispatchEvent(new Event("input"));
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
