// image-crop page module — interactive crop workspace: drag-and-drop upload
// (dropzone + full-window overlay), live preview with a draggable/resizable
// crop rectangle synced to the x/y/width/height fields, and a smart centered
// default box. setup() returns true: this tool owns its ffmpeg wiring —
// fields recompute on `change` only (drag-end / input blur, to avoid ffmpeg
// runs on every keystroke or drag pixel) and the file input is NOT auto-wired
// to run (the preview loader dispatches a change once the image + defaults
// are ready). Loaded by the shared tool.js via the generator's page/custom.js
// hook; styles in custom.css.

export function setup(ctx) {
  const { run, fileInput, fieldInputs, helpers } = ctx;

  setupImageCropCustomUI();

  // Fields trigger ffmpeg only on change (drag-end or input blur) to prevent
  // lag; the overlay's own `input` listeners keep the rectangle live.
  for (const i of fieldInputs) {
    const el = document.getElementById(i.elementId);
    if (el) el.addEventListener("change", run);
  }

  // Deep-link: if ?url= is present, fetch the remote image into the file
  // input and auto-run (replicates the shared ffmpeg path, which this
  // takeover skips).
  const qpUrl = new URLSearchParams(location.search).get("url");
  if (qpUrl && fileInput) {
    loadUrlIntoFile(qpUrl, fileInput, helpers.showError).then((ok) => {
      if (ok) run();
    });
  }

  return true; // full takeover — the shared ffmpeg wiring must not run
}

// Fetch a remote file into the (hidden) file input. Same contract as the
// shared driver's helper: true on success, shows an error and returns false
// when the host blocks cross-origin access.
async function loadUrlIntoFile(url, fileInput, showError) {
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
