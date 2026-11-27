// Interactive crop rectangle over an <img>: drag to move, corner handles to
// resize. Shared by the image-crop tool page (page/custom.js) and any page
// that crops through that tool (e.g. an editor). Coordinates in and out are
// the image's NATURAL pixels; the box maps them to the displayed size.
//
// Styles ship alongside as tool-crop-box.css and are linked on first use.

const MIN_CLIENT_PX = 8; // smallest box edge while dragging, in screen px

// Centered box covering 80% of each dimension (the tool's smart default).
export function defaultRect(width, height) {
  const w = Math.round(width * 0.8) || 1;
  const h = Math.round(height * 0.8) || 1;
  return { x: Math.round((width - w) / 2) || 0, y: Math.round((height - h) / 2) || 0, width: w, height: h };
}

// Clamp a (possibly partial / non-numeric) rect into the image, as integers.
// Missing width/height mean "to the edge".
export function clampRect(rect, width, height) {
  const num = (v, d) => (Number.isFinite(Number(v)) && String(v) !== "" ? Number(v) : d);
  const x = Math.max(0, Math.min(width - 1, Math.round(num(rect.x, 0))));
  const y = Math.max(0, Math.min(height - 1, Math.round(num(rect.y, 0))));
  const w = Math.max(1, Math.min(width - x, Math.round(num(rect.width, width))));
  const h = Math.max(1, Math.min(height - y, Math.round(num(rect.height, height))));
  return { x, y, width: w, height: h };
}

// Resolves once tool-crop-box.css is applied (or failed to load). The box is
// rendered immediately but only lays out correctly with its styles, so every
// box re-renders when this settles. Reuses a matching <link> already in the
// document (e.g. a page that preloads the stylesheet).
let stylesReady = null;
function linkStyles() {
  if (stylesReady) return stylesReady;
  if (typeof document === "undefined") return (stylesReady = Promise.resolve());
  const href = new URL("./tool-crop-box.css", import.meta.url).href;
  let link = Array.from(document.querySelectorAll('link[rel="stylesheet"]')).find((l) => l.href === href);
  if (link && link.sheet) return (stylesReady = Promise.resolve());
  stylesReady = new Promise((resolve) => {
    if (!link) {
      link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = href;
    }
    link.addEventListener("load", () => resolve(), { once: true });
    link.addEventListener("error", () => resolve(), { once: true });
    if (!link.isConnected) document.head.appendChild(link);
  });
  return stylesReady;
}

// container: the element that contains `img`. It becomes the box's host
// (`crop-box-host`: positioned, and clipping the box's page-dimming shade);
// the box is absolutely positioned inside it, offset to wherever the image is
// laid out.
// onChange(rect) fires while dragging; onCommit(rect) once per drag, on release.
export function createCropBox(container, img, { onChange = () => {}, onCommit = () => {} } = {}) {
  const styled = linkStyles();
  const box = document.createElement("div");
  box.className = "crop-overlay-rect";
  for (const h of ["tl", "tr", "bl", "br"]) {
    const handle = document.createElement("div");
    handle.className = `crop-handle crop-handle-${h}`;
    handle.dataset.handle = h;
    box.appendChild(handle);
  }
  container.classList.add("crop-box-host");
  container.appendChild(box);

  let rect = null; // natural px
  const natural = () => ({ w: img.naturalWidth, h: img.naturalHeight });
  const scale = () => ({ x: img.clientWidth / img.naturalWidth, y: img.clientHeight / img.naturalHeight });

  function render() {
    const { w, h } = natural();
    if (!w || !h || !rect) {
      box.hidden = true;
      return;
    }
    box.hidden = false;
    const s = scale();
    box.style.left = `${img.offsetLeft + rect.x * s.x}px`;
    box.style.top = `${img.offsetTop + rect.y * s.y}px`;
    box.style.width = `${rect.width * s.x}px`;
    box.style.height = `${rect.height * s.y}px`;
  }

  function setRect(next) {
    const { w, h } = natural();
    if (!w || !h) return;
    rect = clampRect(next, w, h);
    render();
  }

  let drag = null;
  function onPointerDown(e) {
    if (!rect) return;
    e.preventDefault();
    box.setPointerCapture(e.pointerId);
    const s = scale();
    drag = {
      handle: e.target.dataset.handle || null,
      startX: e.clientX,
      startY: e.clientY,
      // start rect in client px
      x: rect.x * s.x,
      y: rect.y * s.y,
      w: rect.width * s.x,
      h: rect.height * s.y,
    };
  }
  function onPointerMove(e) {
    if (!drag) return;
    e.preventDefault();
    const dx = e.clientX - drag.startX;
    const dy = e.clientY - drag.startY;
    const cw = img.clientWidth;
    const ch = img.clientHeight;
    let { x, y, w, h } = drag;
    if (!drag.handle) {
      x = Math.max(0, Math.min(cw - w, x + dx));
      y = Math.max(0, Math.min(ch - h, y + dy));
    } else {
      if (drag.handle.includes("l")) {
        const nx = Math.max(0, Math.min(x + w - MIN_CLIENT_PX, x + dx));
        w += x - nx;
        x = nx;
      }
      if (drag.handle.includes("r")) w = Math.max(MIN_CLIENT_PX, Math.min(cw - x, w + dx));
      if (drag.handle.includes("t")) {
        const ny = Math.max(0, Math.min(y + h - MIN_CLIENT_PX, y + dy));
        h += y - ny;
        y = ny;
      }
      if (drag.handle.includes("b")) h = Math.max(MIN_CLIENT_PX, Math.min(ch - y, h + dy));
    }
    const s = scale();
    setRect({ x: x / s.x, y: y / s.y, width: w / s.x, height: h / s.y });
    onChange(getRect());
  }
  function onPointerUp(e) {
    if (!drag) return;
    drag = null;
    if (box.hasPointerCapture(e.pointerId)) box.releasePointerCapture(e.pointerId);
    onCommit(getRect());
  }
  box.addEventListener("pointerdown", onPointerDown);
  box.addEventListener("pointermove", onPointerMove);
  box.addEventListener("pointerup", onPointerUp);
  box.addEventListener("pointercancel", onPointerUp);
  window.addEventListener("resize", render);
  let destroyed = false;
  styled.then(() => {
    if (!destroyed) render();
  });

  function getRect() {
    return rect ? { ...rect } : null;
  }

  return {
    setRect,
    getRect,
    // Back to the smart default; returns the new rect.
    reset() {
      const { w, h } = natural();
      if (w && h) setRect(defaultRect(w, h));
      return getRect();
    },
    // Re-layout after the image or its container changed size.
    refresh: render,
    destroy() {
      destroyed = true;
      window.removeEventListener("resize", render);
      box.remove();
      container.classList.remove("crop-box-host");
    },
  };
}
