import { test } from "node:test";
import assert from "node:assert/strict";
import { clampRect, defaultRect } from "../tools/generator/assets/runtime/tool-crop-box.js";

test("defaultRect is a centered 80% box", () => {
  assert.deepEqual(defaultRect(100, 50), { x: 10, y: 5, width: 80, height: 40 });
  assert.deepEqual(defaultRect(1, 1), { x: 0, y: 0, width: 1, height: 1 });
});

test("clampRect keeps the box inside the image", () => {
  assert.deepEqual(clampRect({ x: -5, y: 3, width: 500, height: 2.6 }, 100, 50), { x: 0, y: 3, width: 100, height: 3 });
  assert.deepEqual(clampRect({ x: 99, y: 49, width: 10, height: 10 }, 100, 50), { x: 99, y: 49, width: 1, height: 1 });
});

test("clampRect treats empty/non-numeric sizes as 'to the edge'", () => {
  assert.deepEqual(clampRect({ x: "10", y: "", width: "", height: "abc" }, 100, 50), { x: 10, y: 0, width: 90, height: 50 });
});
