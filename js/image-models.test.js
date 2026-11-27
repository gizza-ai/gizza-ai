import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  IMAGE_MODELS,
  adapterInfo,
  defaultModel,
  deviceFit,
  resolveVariant,
} from "../tools/generator/assets/runtime/image-models.js";

const byId = (id) => IMAGE_MODELS.find((m) => m.id === id);

test("defaultModel is SD-Turbo, resolved to nof16q8, without shader-f16", () => {
  const info = { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 31 };
  const m = defaultModel(info);
  assert.equal(m.engine, "sd-turbo");
  assert.equal(resolveVariant(m, info).name, "nof16q8");
});

test("shader-f16 does not select fp16 while it is unverified", () => {
  const info = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
  const m = defaultModel(info);
  assert.equal(m.engine, "sd-turbo");
  assert.equal(m.variants.fp16.verified, false);
  assert.equal(resolveVariant(m, info).name, "nof16q8");
});

test("defaultModel falls back to Janus when SD-Turbo is risky on a small-buffer adapter", () => {
  const info = { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 29 };
  assert.equal(deviceFit(byId("sd-turbo"), info).level, "risky");
  assert.equal(defaultModel(info).id, "janus-pro-1b");
});

test("defaultModel still returns the recommended entry without webgpu", () => {
  assert.equal(defaultModel({ webgpu: false, shaderF16: false, maxBufferSize: 0 }).id, "sd-turbo");
});

test("deviceFit is unsupported without webgpu", () => {
  for (const m of IMAGE_MODELS) {
    const fit = deviceFit(m, { webgpu: false, shaderF16: false, maxBufferSize: 0 });
    assert.equal(fit.level, "unsupported");
    assert.ok(fit.note);
  }
});

test("deviceFit is ok on a capable adapter and risky on a small buffer limit", () => {
  const sd = byId("sd-turbo");
  assert.equal(deviceFit(sd, { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 31 }).level, "ok");
  assert.equal(deviceFit(sd, { webgpu: true, shaderF16: false, maxBufferSize: 2 ** 20 }).level, "risky");
});

test("Janus descriptor matches the imagine tool's page model", () => {
  const meta = readFileSync(new URL("../blocks/imagine/page/meta.toml", import.meta.url), "utf8");
  const field = (k) => meta.match(new RegExp(`^${k}\\s*=\\s*"?([^"\\n]+)"?`, "m"))[1].trim();
  const janus = byId("janus-pro-1b");
  const info = { webgpu: true, shaderF16: true, maxBufferSize: 2 ** 31 };
  const d = resolveVariant(janus, info).descriptor;
  assert.equal(resolveVariant(janus, info).name, "f16");
  assert.deepEqual(
    { task: d.task, id: d.id, revision: d.revision, download_mb: d.download_mb },
    { task: field("task"), id: field("id"), revision: field("revision"), download_mb: Number(field("download_mb")) },
  );
  assert.equal(d.engine, "janus");
  assert.equal(d.variant, "f16");
  assert.deepEqual(d.output, { w: 384, h: 384 });
  assert.equal(resolveVariant(janus, { ...info, shaderF16: false }).name, "f32");
});

test("resolveVariant returns null when no variant fits, and for no webgpu", () => {
  assert.equal(resolveVariant(byId("sd-turbo"), { webgpu: false, shaderF16: false, maxBufferSize: 0 }), null);
});

test("every entry has license name and url, and shape matches custom entries", () => {
  for (const m of IMAGE_MODELS) {
    assert.ok(m.license.name && m.license.url, m.id);
    for (const k of ["id", "label", "tagline", "engine", "repo", "revision", "output", "variants"]) {
      assert.ok(m[k], `${m.id}.${k}`);
    }
    for (const v of Object.values(m.variants)) {
      assert.equal(typeof v.requires.shaderF16, "boolean");
      assert.equal(typeof v.download_mb, "number");
      assert.equal(typeof v.verified, "boolean");
    }
  }
  assert.equal(IMAGE_MODELS.filter((m) => m.recommended).length, 1);
});

test("adapterInfo reads webgpu, shader-f16 and maxBufferSize", async () => {
  const nav = {
    gpu: {
      requestAdapter: async () => ({
        features: new Set(["shader-f16"]),
        limits: { maxBufferSize: 123 },
      }),
    },
  };
  assert.deepEqual(await adapterInfo(nav), { webgpu: true, shaderF16: true, maxBufferSize: 123 });
  assert.deepEqual(await adapterInfo({}), { webgpu: false, shaderF16: false, maxBufferSize: 0 });
  assert.deepEqual(await adapterInfo({ gpu: { requestAdapter: async () => null } }), {
    webgpu: false,
    shaderF16: false,
    maxBufferSize: 0,
  });
});
