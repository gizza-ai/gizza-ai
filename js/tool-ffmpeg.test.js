import { test } from "node:test";
import assert from "node:assert/strict";
import { inputNameFor, dataUrlFor } from "../site/tool-ffmpeg.js";

test("inputNameFor derives in.<ext> from a filename", () => {
  assert.equal(inputNameFor("cat.PNG"), "in.png");
  assert.equal(inputNameFor("clip.mp4"), "in.mp4");
  assert.equal(inputNameFor("noext"), "in.bin");
});

test("dataUrlFor builds a base64 data URL", () => {
  assert.equal(dataUrlFor("image/png", "AAAA"), "data:image/png;base64,AAAA");
});
