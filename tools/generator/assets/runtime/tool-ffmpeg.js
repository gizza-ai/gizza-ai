// ffmpeg tool-page helpers + run flow. Pure helpers are unit-tested; runFfmpeg
// is wired by tool.js for runtime === "ffmpeg" tools.
//
// Constraints (kept simple on purpose — the new-tool skill clones this shape):
//   * exactly ONE file input (the first source="file" input);
//   * the OUTPUT media type is inferred from the produced filename's extension
//     (`out_name`, which the tool's web/build_argv chooses), NOT the input file's
//     MIME — so a format/container change (e.g. video transcode) renders correctly.

export function inputNameFor(filename) {
  const dot = filename.lastIndexOf(".");
  const ext = dot >= 0 ? filename.slice(dot + 1).toLowerCase() : "bin";
  return `in.${ext || "bin"}`;
}

const EXT_MIME = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp",
  gif: "image/gif", bmp: "image/bmp", avif: "image/avif",
  mp4: "video/mp4", m4v: "video/mp4", webm: "video/webm", mov: "video/quicktime", mkv: "video/x-matroska",
  mp3: "audio/mpeg", wav: "audio/wav", ogg: "audio/ogg", flac: "audio/flac",
  aac: "audio/aac", m4a: "audio/mp4", m4r: "audio/mp4", opus: "audio/opus",
};

// MIME for the produced file, from its extension (e.g. "out.mp4" -> "video/mp4").
export function mimeForOutput(outName) {
  const dot = outName.lastIndexOf(".");
  const ext = dot >= 0 ? outName.slice(dot + 1).toLowerCase() : "";
  return EXT_MIME[ext] || "application/octet-stream";
}

function b64ToBytes(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

// cfg: the tool's page config (window.GIZZA_TOOL or tool.json `page`); mod: the
// loaded web-wasm module; execBytes: `ffmpegExecBytes` from ffmpeg.js.
// fieldArgs: the field input values already coerced (numbers where numeric).
// Returns {ok: true, blob, mime, outName} or {ok: false, error}.
export async function runFfmpeg(cfg, mod, execBytes, file, fieldArgs) {
  const inName = inputNameFor(file.name);
  const bytes = new Uint8Array(await file.arrayBuffer());

  // The web wasm builds the argv (pure, shared with the chat block's core).
  // Signature: build_argv(...fieldArgs, inName) ->
  //   { argv: string[], out_name: string, inputs?: [name, base64][] }.
  // `inputs` (optional) are extra virtual-FS files — a bundled font + a text
  // file for drawtext-style tools — written alongside the uploaded media so
  // the shared filtergraph resolves the same way it does on the CLI.
  let plan;
  try {
    plan = mod[cfg.export](...fieldArgs, inName);
  } catch (e) {
    return { ok: false, error: typeof e === "string" ? e : e && e.message ? e.message : "invalid args" };
  }
  const extraInputs = Array.isArray(plan.inputs)
    ? plan.inputs.map(([name, b64]) => ({ name, bytes: b64ToBytes(b64) }))
    : [];
  const resp = await execBytes(plan.argv, [{ name: inName, bytes }, ...extraInputs], plan.out_name);
  if (resp.exit_code !== 0 || !resp.output || !resp.output.length) {
    const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
    return { ok: false, error: snippet };
  }
  const mime = mimeForOutput(plan.out_name);
  return { ok: true, blob: new Blob([resp.output], { type: mime }), mime, outName: plan.out_name };
}
