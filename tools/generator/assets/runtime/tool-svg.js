// Pure helpers for the format="svg" output path. Kept out of tool.js — that
// module runs on import (reads window.GIZZA_TOOL, touches the DOM), so it can't
// be imported by node --test. Same split as tool-ffmpeg.js.

const PREFIX = "data:image/svg+xml;base64,";

/**
 * Build a data: URI for an SVG string, for use as an <img> src.
 *
 * Rendering SVG through <img> (rather than innerHTML) means the markup cannot
 * execute script, so no sanitizer is needed and no block has to be trusted to
 * escape user text correctly.
 *
 * Encodes via TextEncoder because btoa() throws on any code point > U+00FF and
 * chart labels carry arbitrary user text. Returns "" for empty/blank/non-string
 * input so the caller can fall back to the text pane instead of setting a
 * broken src.
 */
export function svgDataUrl(svg) {
  if (typeof svg !== "string" || !svg.trim()) return "";
  const bytes = new TextEncoder().encode(svg);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return PREFIX + btoa(bin);
}
