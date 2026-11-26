// MIME type of a tool's produced file, from its name's extension — shared by
// every media runtime (ffmpeg, wasm-image) so they label results identically.

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
