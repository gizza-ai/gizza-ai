import { test } from "node:test";
import assert from "node:assert/strict";
import { resolve } from "./routing.mjs";

test("apex serves the app unchanged", () => {
  assert.deepEqual(resolve("gizza.ai", "/"), { type: "app", path: "/" });
  assert.deepEqual(resolve("www.gizza.ai", "/foo"), { type: "app", path: "/foo" });
});

test("tool subdomain rewrites to /tools/<sub>/...", () => {
  assert.deepEqual(resolve("calculator.gizza.ai", "/"), {
    type: "tool",
    path: "/tools/calculator/index.html",
  });
  assert.deepEqual(resolve("clock.gizza.ai", "/tool.css"), {
    type: "tool",
    path: "/tools/clock/tool.css",
  });
});

test("host with port is handled", () => {
  assert.deepEqual(resolve("calculator.gizza.ai:443", "/"), {
    type: "tool",
    path: "/tools/calculator/index.html",
  });
});

test("localhost and pages.dev serve the app", () => {
  assert.equal(resolve("localhost", "/").type, "app");
  assert.equal(resolve("gizza-ai.pages.dev", "/").type, "app");
});
