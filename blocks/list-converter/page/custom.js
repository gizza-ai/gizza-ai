// list-converter page module — rebuilds the template-generated inputs into a
// dual-pane layout (input + settings on the left, formatted output + stats +
// swap/reset/copy actions on the right). setup() returns undefined: the
// shared driver's generic wiring (deep-links, live recompute on input/change)
// still runs afterwards and drives the wasm export as usual; renderResult/
// renderError route the result into the right-hand pane. Loaded by the shared
// tool.js via the generator's page/custom.js hook; styles in custom.css.

export function setup(ctx) {
  const out = ctx.out;

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
  const widget = document.querySelector(".tool-widget");
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
    // Capture the value first: the shared driver applies URL-prefills before
    // setup runs, and clearing innerHTML would reset the select.
    const prev = inSep.value;
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
    if (prev) inSep.value = prev;
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
    // Capture the value first: the shared driver applies URL-prefills before
    // setup runs, and clearing innerHTML would reset the select.
    const prev = outFormat.value;
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
    if (prev) outFormat.value = prev;
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
    // Capture the value first: the shared driver applies URL-prefills before
    // setup runs, and clearing innerHTML would reset the select.
    const prev = sortMode.value;
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
    if (prev) sortMode.value = prev;
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
    // Capture the value first: the shared driver applies URL-prefills before
    // setup runs, and clearing innerHTML would reset the select.
    const prev = caseTransform.value;
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
    if (prev) caseTransform.value = prev;
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
  // No `return true` — the shared driver's generic wiring (deep-links + live
  // recompute on the rebuilt inputs) still runs after this setup.
}

export function renderResult(value, ctx) {
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

  if (!inInput) return true;

  const text = inInput.value;
  if (!text.trim()) {
    const statIn = document.getElementById("list-conv-stat-in");
    const statOut = document.getElementById("list-conv-stat-out");
    const diffContainer = document.getElementById("list-conv-stat-diff-container");
    if (statIn) statIn.textContent = "0";
    if (statOut) statOut.textContent = "0";
    if (diffContainer) diffContainer.style.display = "none";
    return true;
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
  return true;
}

export function renderError(message, ctx) {
  const out = ctx.out;
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
    return true;
  }
  out.classList.remove("error");
  if (outputTextarea) {
    outputTextarea.value = message;
  }
  return true;
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
