import { test } from "node:test";
import assert from "node:assert/strict";
import { clampRect, createCropBox, defaultRect } from "../tools/generator/assets/runtime/tool-crop-box.js";

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

// Minimal DOM stand-ins: just what createCropBox touches.
function fakeElement(tag) {
  const listeners = {};
  return {
    tag,
    dataset: {},
    style: {},
    children: [],
    isConnected: false,
    appendChild(child) { this.children.push(child); child.isConnected = true; return child; },
    addEventListener(type, fn) { (listeners[type] ||= []).push(fn); },
    removeEventListener() {},
    remove() { this.isConnected = false; },
    fire(type) { for (const fn of listeners[type] || []) fn({ type }); },
  };
}

test("the box re-renders once its stylesheet loads", async () => {
  const head = fakeElement("head");
  globalThis.document = {
    head,
    createElement: fakeElement,
    querySelectorAll: () => [],
  };
  globalThis.window = { addEventListener() {}, removeEventListener() {} };
  try {
    const img = { naturalWidth: 100, naturalHeight: 50, clientWidth: 200, clientHeight: 100, offsetLeft: 0, offsetTop: 0 };
    const container = fakeElement("div");
    const crop = createCropBox(container, img);
    crop.setRect({ x: 10, y: 5, width: 80, height: 40 });
    const box = container.children[0];
    assert.equal(box.style.left, "20px");

    // Unstyled layout differs; once the CSS applies, the image moves and the
    // box must follow without any other trigger.
    const link = head.children.find((el) => el.tag === "link");
    assert.ok(link, "stylesheet linked on first use");
    assert.match(link.href, /tool-crop-box\.css$/);
    img.offsetLeft = 7;
    img.offsetTop = 3;
    link.fire("load");
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert.equal(box.style.left, "27px");
    assert.equal(box.style.top, "13px");

    // A second box reuses the same link and still renders once styles are ready.
    const container2 = fakeElement("div");
    createCropBox(container2, img).setRect({ x: 0, y: 0, width: 10, height: 10 });
    assert.equal(head.children.filter((el) => el.tag === "link").length, 1);
  } finally {
    delete globalThis.document;
    delete globalThis.window;
  }
});
