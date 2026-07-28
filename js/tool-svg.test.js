import { test } from "node:test";
import assert from "node:assert/strict";
import { svgDataUrl } from "../tools/generator/assets/runtime/tool-svg.js";

const PREFIX = "data:image/svg+xml;base64,";

test("svgDataUrl builds a base64 data URI that round-trips", () => {
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><rect width="4" height="4"/></svg>';
  const url = svgDataUrl(svg);
  assert.ok(url.startsWith(PREFIX));
  assert.equal(Buffer.from(url.slice(PREFIX.length), "base64").toString("utf8"), svg);
});

test("svgDataUrl handles non-ASCII label text", () => {
  // Chart titles/labels carry arbitrary user text; btoa() alone throws on any
  // code point > U+00FF, which is why the helper encodes via TextEncoder.
  const svg = '<svg xmlns="http://www.w3.org/2000/svg"><text>café 東京</text></svg>';
  const url = svgDataUrl(svg);
  assert.equal(Buffer.from(url.slice(PREFIX.length), "base64").toString("utf8"), svg);
});

test("svgDataUrl returns empty string for empty, blank or non-string input", () => {
  assert.equal(svgDataUrl(""), "");
  assert.equal(svgDataUrl("   \n"), "");
  assert.equal(svgDataUrl(null), "");
  assert.equal(svgDataUrl(undefined), "");
});
