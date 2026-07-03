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

export function dataUrlFor(mime, b64) {
  return `data:${mime};base64,${b64}`;
}

const EXT_MIME = {
  png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", webp: "image/webp",
  gif: "image/gif", bmp: "image/bmp", avif: "image/avif",
  mp4: "video/mp4", m4v: "video/mp4", webm: "video/webm", mov: "video/quicktime", mkv: "video/x-matroska",
  mp3: "audio/mpeg", wav: "audio/wav", ogg: "audio/ogg", flac: "audio/flac",
  aac: "audio/aac", m4a: "audio/mp4", opus: "audio/opus",
};

// MIME for the produced file, from its extension (e.g. "out.mp4" -> "video/mp4").
export function mimeForOutput(outName) {
  const dot = outName.lastIndexOf(".");
  const ext = dot >= 0 ? outName.slice(dot + 1).toLowerCase() : "";
  return EXT_MIME[ext] || "application/octet-stream";
}

function bytesToB64(u8) {
  let s = "";
  const chunk = 0x8000;
  for (let i = 0; i < u8.length; i += chunk) {
    s += String.fromCharCode.apply(null, u8.subarray(i, i + chunk));
  }
  return btoa(s);
}

// cfg: window.GIZZA_TOOL; mod: the loaded web-wasm module; ffmpegExec: from ./ffmpeg.js.
// fieldArgs: the field input values already coerced (numbers where numeric).
// Returns {ok, dataUrl?, mime?, outName?, error?}.
export async function runFfmpeg(cfg, mod, ffmpegExec, file, fieldArgs) {
  const inName = inputNameFor(file.name);
  const buf = new Uint8Array(await file.arrayBuffer());
  const bytes_b64 = bytesToB64(buf);

  // The web wasm builds the argv (pure, shared with the chat block's core).
  // Signature: build_argv(...fieldArgs, inName) -> { argv: string[], out_name: string }.
  let plan;
  try {
    plan = mod[cfg.export](...fieldArgs, inName);
  } catch (e) {
    return { ok: false, error: typeof e === "string" ? e : e && e.message ? e.message : "invalid args" };
  }
  const resp = await ffmpegExec(
    JSON.stringify(plan.argv),
    JSON.stringify([{ name: inName, bytes_b64 }]),
    plan.out_name
  );
  if (resp.exit_code !== 0 || !resp.output_b64) {
    const snippet = (resp.log || "").split("\n").filter(Boolean).slice(-1)[0] || "ffmpeg failed";
    return { ok: false, error: snippet };
  }
  const mime = mimeForOutput(plan.out_name);
  return { ok: true, dataUrl: dataUrlFor(mime, resp.output_b64), mime, outName: plan.out_name };
}
