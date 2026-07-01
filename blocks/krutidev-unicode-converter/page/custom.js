// krutidev-unicode-converter page module — a bidirectional dual-pane
// converter (Kruti Dev ⇔ Unicode) with its own copy/clear chrome, wired
// directly to the wasm module's two exports. setup() returns true: this tool
// owns ALL its wiring (the generic single-export compute loop does not fit a
// two-way converter). Loaded by the shared tool.js via the generator's
// page/custom.js hook; styles in custom.css.

export function setup(ctx) {
  const mod = ctx.mod;
  const out = ctx.out;

  // Dual-pane UI ships its own copy/clear buttons — drop the generic chrome.
  document.getElementById("tool-reset")?.remove();
  document.getElementById("tool-copy-output")?.remove();

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

  return true; // full takeover — the generic compute wiring must not run
}
