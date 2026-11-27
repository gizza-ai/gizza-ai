// Field-value coercion shared by every surface that calls a tool's web wasm
// with positional args: the tool page (tool.js), tool-run.js, and any page that
// drives a published tool (e.g. an editor). One rule, one place.
//
// A value becomes a Number ONLY when the param's DECLARED schema type is
// numeric (`input.numeric`, baked from manifest.json tool.parameters by the
// generator) — so wasm-bindgen f64 params marshal correctly, while a string
// param whose value happens to be digits (a bare hex color "112233", a
// numeric-looking label/code) stays a string. Empty and non-finite values pass
// through as the raw string; the wasm function applies its own defaults.

export function coerceField(input, raw) {
  const v = raw == null ? "" : String(raw);
  if (input && input.numeric && v !== "") {
    const n = Number(v);
    if (Number.isFinite(n)) return n;
  }
  return v;
}

// Positional args for a tool's web export: every `source === "field"` input, in
// declared order, read through `read(input)` and coerced. File inputs are not
// args here — the runtime appends the input file (name or bytes) itself.
export function fieldArgs(inputs, read) {
  return inputs.filter((i) => i.source === "field").map((i) => coerceField(i, read(i)));
}
