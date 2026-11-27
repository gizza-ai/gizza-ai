// Overall download progress for a multi-file model load, built from
// Transformers.js' per-file progress events. Shipped next to model-worker.js
// in /tools/_model-runtime/ (dependency-free so it is unit-testable in Node).

// Returns a progress_callback. `onFraction(fraction, loaded, total)` fires on
// every byte update.
//
// A model's own from_pretrained looks up every weight file's size before
// downloading and reports the whole load as `progress_total` events; when
// those arrive they are the measure. Otherwise (a processor's small files, an
// older Transformers.js) files announce their size only when their own
// download starts, so the sizes seen so far undercount the load — the small
// config files finish before the multi-GB weights report theirs. That byte
// ratio is measured against `expectedBytes` (the model's declared first-use
// download size) whenever it is larger than what has been seen. Either way the
// fraction is held at its high-water mark, so the bar never moves backwards.
export function downloadTracker(onFraction, expectedBytes = 0) {
  const files = new Map();
  let reported = 0;
  let overall = null; // the latest progress_total { loaded, total }
  const report = (loaded, total) => {
    reported = Math.max(reported, Math.min(1, loaded / total));
    onFraction(reported, loaded, total);
  };
  return (event) => {
    if (!event) return;
    if (event.status === "progress_total") {
      if (!(Number.isFinite(event.total) && event.total > 0)) return;
      overall = { loaded: event.loaded, total: event.total };
      report(overall.loaded, overall.total);
      return;
    }
    if (overall || !event.file) return;
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
    const denominator = Math.max(total, expectedBytes);
    if (denominator <= 0) return;
    report(loaded, denominator);
  };
}
