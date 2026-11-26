import { test } from "node:test";
import assert from "node:assert/strict";
import { inputNameFor, mimeForOutput, runFfmpeg } from "../tools/generator/assets/runtime/tool-ffmpeg.js";

test("inputNameFor derives in.<ext> from a filename", () => {
  assert.equal(inputNameFor("cat.PNG"), "in.png");
  assert.equal(inputNameFor("clip.mp4"), "in.mp4");
  assert.equal(inputNameFor("noext"), "in.bin");
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

function fakeFile(name, bytes) {
  return { name, arrayBuffer: async () => new Uint8Array(bytes).buffer };
}

test("runFfmpeg passes raw bytes in and returns a typed Blob", async () => {
  const calls = [];
  const mod = {
    build_argv: (width, inName) => {
      calls.push(["argv", width, inName]);
      return { argv: ["-i", inName, "out.png"], out_name: "out.png", inputs: [["font.ttf", "AQI="]] };
    },
  };
  const execBytes = async (args, inputs, outName) => {
    calls.push(["exec", args, inputs.map((i) => [i.name, Array.from(i.bytes)]), outName]);
    return { exit_code: 0, output: new Uint8Array([1, 2, 3]), log: "" };
  };
  const r = await runFfmpeg({ export: "build_argv" }, mod, execBytes, fakeFile("cat.jpg", [9, 8]), [640]);
  assert.equal(r.ok, true);
  assert.equal(r.outName, "out.png");
  assert.equal(r.mime, "image/png");
  assert.equal(r.blob.type, "image/png");
  assert.deepEqual(Array.from(new Uint8Array(await r.blob.arrayBuffer())), [1, 2, 3]);
  assert.deepEqual(calls[0], ["argv", 640, "in.jpg"]);
  assert.deepEqual(calls[1], ["exec", ["-i", "in.jpg", "out.png"], [["in.jpg", [9, 8]], ["font.ttf", [1, 2]]], "out.png"]);
});

test("runFfmpeg reports the last log line when ffmpeg fails", async () => {
  const mod = { build_argv: () => ({ argv: [], out_name: "out.png" }) };
  const execBytes = async () => ({ exit_code: 1, output: null, log: "a\nInvalid argument\n" });
  const r = await runFfmpeg({ export: "build_argv" }, mod, execBytes, fakeFile("x.png", [1]), []);
  assert.deepEqual(r, { ok: false, error: "Invalid argument" });
});

test("runFfmpeg surfaces a build_argv error string", async () => {
  const mod = { build_argv: () => { throw "width must be positive"; } };
  const r = await runFfmpeg({ export: "build_argv" }, mod, async () => assert.fail("no exec"), fakeFile("x.png", [1]), [-1]);
  assert.deepEqual(r, { ok: false, error: "width must be positive" });
});
