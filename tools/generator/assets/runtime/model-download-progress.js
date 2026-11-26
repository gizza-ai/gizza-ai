// Overall download progress for a multi-file model load, built from
// Transformers.js' per-file progress events. Shipped next to model-worker.js
// in /tools/_model-runtime/ (dependency-free so it is unit-testable in Node).

// Returns a progress_callback. `onFraction(fraction, loaded, total)` fires on
// every byte update. Files announce their size only when their own download
// starts, so the byte ratio alone would drop each time a new file joins the
// total; the reported fraction is held at its high-water mark instead, so the
// bar never moves backwards.
export function downloadTracker(onFraction) {
  const files = new Map();
  let reported = 0;
  return (event) => {
    if (!event || !event.file) return;
    if (event.status === "progress" && Number.isFinite(event.total) && event.total > 0) {
      files.set(event.file, { loaded: event.loaded, total: event.total });
    } else if (event.status === "done" && files.has(event.file)) {
      const f = files.get(event.file);
      files.set(event.file, { loaded: f.total, total: f.total });
    } else {
      return;
    }
    let loaded = 0;
    let total = 0;
    for (const f of files.values()) {
      loaded += f.loaded;
      total += f.total;
    }
    if (total <= 0) return;
    reported = Math.max(reported, Math.min(1, loaded / total));
    onFraction(reported, loaded, total);
  };
}
