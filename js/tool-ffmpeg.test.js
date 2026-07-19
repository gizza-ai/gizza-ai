import { test } from "node:test";
import assert from "node:assert/strict";
import { inputNameFor, dataUrlFor, mimeForOutput } from "../tools/generator/assets/runtime/tool-ffmpeg.js";

test("inputNameFor derives in.<ext> from a filename", () => {
  assert.equal(inputNameFor("cat.PNG"), "in.png");
  assert.equal(inputNameFor("clip.mp4"), "in.mp4");
  assert.equal(inputNameFor("noext"), "in.bin");
});

test("dataUrlFor builds a base64 data URL", () => {
  assert.equal(dataUrlFor("image/png", "AAAA"), "data:image/png;base64,AAAA");
});

test("mimeForOutput infers from the output extension, not the input", () => {
  assert.equal(mimeForOutput("out.png"), "image/png");
  assert.equal(mimeForOutput("out.mp4"), "video/mp4");
  assert.equal(mimeForOutput("out.webm"), "video/webm");
  // .m4r (iPhone ringtone) is AAC in an MP4 container — audio/mp4, so the
  // page's <audio> preview can play it (audio-ringtone writes out.m4r).
  assert.equal(mimeForOutput("out.m4r"), "audio/mp4");
  assert.equal(mimeForOutput("out.unknown"), "application/octet-stream");
});
