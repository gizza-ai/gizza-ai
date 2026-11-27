import { test } from "node:test";
import assert from "node:assert/strict";
import { coerceField, fieldArgs } from "../tools/generator/assets/runtime/tool-fields.js";

test("coerceField converts only declared-numeric, finite values", () => {
  assert.equal(coerceField({ numeric: true }, "12"), 12);
  assert.equal(coerceField({ numeric: true }, "-0.5"), -0.5);
  assert.equal(coerceField({ numeric: true }, ""), "");
  assert.equal(coerceField({ numeric: true }, "abc"), "abc");
  assert.equal(coerceField({}, "112233"), "112233");
  assert.equal(coerceField({ numeric: true }, undefined), "");
});

test("fieldArgs keeps declared field order and skips file inputs", () => {
  const inputs = [
    { name: "image", source: "file" },
    { name: "width", source: "field", numeric: true },
    { name: "fit", source: "field" },
  ];
  const values = { width: "640", fit: "cover" };
  assert.deepEqual(fieldArgs(inputs, (i) => values[i.name]), [640, "cover"]);
});
