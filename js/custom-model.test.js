import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { inspectCustomModel } from "../tools/generator/assets/runtime/custom-model.js";

const SHA = "0123456789abcdef0123456789abcdef01234567";
const fx = (name) => JSON.parse(readFileSync(new URL(`./fixtures/hf/${name}.json`, import.meta.url), "utf8"));
const json = (body, init = {}) =>
  new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" }, ...init });

// Mock HF: routes maps a URL substring to a Response factory; records every URL.
function mockFetch(routes) {
  const seen = [];
  const fn = async (url) => {
    seen.push(String(url));
    for (const [needle, make] of routes) if (String(url).includes(needle)) return make(String(url));
    return new Response("{}", { status: 404 });
  };
  fn.seen = seen;
  return fn;
}
const noResolve = (f) => assert.equal(f.seen.filter((u) => u.includes("/resolve/")).length, 0);
const INFO = { webgpu: true, shaderF16: true, maxBufferSize: 4e9 };

function repoRoutes(rev, tree) {
  return [
    ["/tree/", () => json(tree)],
    ["/revision/", () => json(rev)],
  ];
}

test("janus repo is recognised with per-variant sizes", async () => {
  const f = mockFetch(repoRoutes(fx("janus-rev"), fx("janus-tree")));
  const r = await inspectCustomModel("acme/janus-web", { fetch: f, info: INFO });
  assert.equal(r.ok, true);
  assert.equal(r.recognized, "format");
  const e = r.entry;
  assert.equal(e.id, `custom:acme/janus-web@${SHA}`);
  assert.equal(e.engine, "janus");
  assert.equal(e.repo, "acme/janus-web");
  assert.equal(e.revision, SHA);
  assert.deepEqual(e.output, { w: 384, h: 384 });
  assert.equal(e.license.name, "mit");
  assert.equal(e.variants.f16.requires.shaderF16, true);
  assert.equal(e.variants.f32.requires.shaderF16, false);
  assert.equal(e.variants.f16.verified, false);
  // f16: q4(1) + q4f16(1) + fp16(4)x3 + fp32(8) of 10 MB data + 6 x 1 MB graphs
  assert.equal(e.variants.f16.download_mb, Math.round((6e6 + 10e6 * (1 + 1 + 12 + 8)) / 1e6));
  // f32: q8(2) + q4(1)x3 + fp32(8)x2
  assert.equal(e.variants.f32.download_mb, Math.round((6e6 + 10e6 * (2 + 3 + 16)) / 1e6));
  noResolve(f);
});

test("gizza SD repo is recognised and sized from the variant files", async () => {
  const f = mockFetch(repoRoutes(fx("sd-rev"), fx("sd-tree")));
  const r = await inspectCustomModel("acme/sd-web@main", { fetch: f, info: INFO });
  assert.equal(r.ok, true);
  const e = r.entry;
  assert.equal(e.engine, "sd-turbo");
  assert.equal(e.id, `custom:acme/sd-web@${SHA}`);
  assert.deepEqual(e.output, { w: 512, h: 512 });
  assert.equal(e.variants.nof16q8.requires.shaderF16, false);
  assert.equal(e.variants.fp16.requires.shaderF16, true);
  // tokenizer 2 MB + manifest 3 kB + 3 graphs 0.1 MB + 3 data files
  assert.equal(e.variants.nof16q8.download_mb, Math.round((2e6 + 3e3 + 0.3e6 + 300e6) / 1e6));
  assert.equal(e.variants.fp16.download_mb, Math.round((2e6 + 3e3 + 0.3e6 + 600e6) / 1e6));
  assert.ok(f.seen[0].includes("/revision/main"));
  assert.ok(f.seen.some((u) => u.includes(`/tree/${SHA}?recursive=true`)));
  noResolve(f);
});

test("plain diffusers repo is not a supported format", async () => {
  const f = mockFetch(repoRoutes(fx("diffusers-rev"), fx("diffusers-tree")));
  const r = await inspectCustomModel("acme/sd-plain", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.match(r.reason, /isn't a supported format/);
  noResolve(f);
});

test("gated repo needs a login", async () => {
  const f = mockFetch([["/revision/", () => json(fx("gated-rev"))]]);
  const r = await inspectCustomModel("acme/gated", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.equal(r.reason, "needs a Hugging Face login, which gizza doesn't support");
  noResolve(f);
});

test("404 is not found", async () => {
  const f = mockFetch([]);
  const r = await inspectCustomModel("acme/nope", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.match(r.reason, /not found/);
  noResolve(f);
});

test("429 is rate limiting", async () => {
  const f = mockFetch([["/revision/", () => new Response("", { status: 429 })]]);
  const r = await inspectCustomModel("acme/x", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.equal(r.reason, "Hugging Face is rate-limiting; try again in a minute");
  noResolve(f);
});

test("paginated tree reads every page", async () => {
  const tree = fx("sd-tree");
  const pages = [tree.slice(0, 4), tree.slice(4, 9), tree.slice(9)];
  const f = mockFetch([
    ["page=3", () => json(pages[2])],
    ["page=2", () =>
      json(pages[1], { headers: { Link: '<https://huggingface.co/api/models/acme/sd-web/tree/x?recursive=true&page=3>; rel="next"' } })],
    ["/tree/", () =>
      json(pages[0], { headers: { Link: '<https://huggingface.co/api/models/acme/sd-web/tree/x?recursive=true&page=2>; rel="next", <https://x/last>; rel="last"' } })],
    ["/revision/", () => json(fx("sd-rev"))],
  ]);
  const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
  assert.equal(r.ok, true);
  assert.equal(f.seen.filter((u) => u.includes("/tree/")).length, 3);
  noResolve(f);
});

test("malformed specs are rejected without a request", async () => {
  const f = mockFetch([]);
  const r = await inspectCustomModel("not a repo", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.equal(f.seen.length, 0);
});

test("an SD repo without manifest.json is not a supported format (the worker sizes from it)", async () => {
  const tree = fx("sd-tree").filter((f) => f.path !== "manifest.json");
  const f = mockFetch(repoRoutes(fx("sd-rev"), tree));
  const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
  assert.equal(r.ok, false);
  assert.match(r.reason, /isn't a supported format/);
});

test("a repo with no license says no license stated, not a license name", async () => {
  for (const cardData of [undefined, {}, { license: "" }, { license: [] }]) {
    const f = mockFetch(repoRoutes({ ...fx("sd-rev"), cardData }, fx("sd-tree")));
    const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
    assert.equal(r.entry.license.name, "no license stated", JSON.stringify(cardData));
    assert.match(r.entry.license.summary, /states no license/);
    assert.doesNotMatch(r.entry.license.summary, /declared by the repo/);
  }
  const f = mockFetch(repoRoutes({ ...fx("sd-rev"), cardData: { license: ["mit", "apache-2.0"] } }, fx("sd-tree")));
  const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
  assert.equal(r.entry.license.name, "mit, apache-2.0");
});

test("401/403 may be a typo or a locked repo, and the message says both", async () => {
  for (const status of [401, 403]) {
    const f = mockFetch([["/revision/", () => new Response("", { status })]]);
    const r = await inspectCustomModel("acme/typo", { fetch: f, info: INFO });
    assert.equal(r.ok, false);
    assert.equal(r.reason, "not found, or private/gated (needs a Hugging Face login, which gizza doesn't support)");
  }
});

test("a revision whose commit is not a 40-hex sha is refused before any tree request", async () => {
  for (const sha of [undefined, "", "main", "../../evil", `${SHA}/x`, SHA.toUpperCase()]) {
    const f = mockFetch(repoRoutes({ ...fx("sd-rev"), sha }, fx("sd-tree")));
    const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
    assert.equal(r.ok, false, String(sha));
    assert.match(r.reason, /didn't return a commit/);
    assert.equal(f.seen.filter((u) => u.includes("/tree/")).length, 0);
  }
});

test("tree pagination never leaves huggingface.co", async () => {
  for (const next of ["https://evil.example/api/models/acme/sd-web/tree/x?page=2", "http://huggingface.co/api/x", "//evil.example/x", "not a url"]) {
    const f = mockFetch([
      ["/tree/", () => json(fx("sd-tree").slice(0, 4), { headers: { Link: `<${next}>; rel="next"` } })],
      ["/revision/", () => json(fx("sd-rev"))],
    ]);
    const r = await inspectCustomModel("acme/sd-web", { fetch: f, info: INFO });
    assert.equal(r.ok, false, next);
    assert.match(r.reason, /unexpected next-page link/);
    assert.ok(f.seen.every((u) => u.startsWith("https://huggingface.co/")), f.seen.join(" "));
  }
});

test("dot-only repo names are rejected without a request", async () => {
  for (const spec of ["owner/..", "owner/.", "owner/...@main"]) {
    const f = mockFetch([]);
    const r = await inspectCustomModel(spec, { fetch: f, info: INFO });
    assert.equal(r.ok, false, spec);
    assert.equal(f.seen.length, 0, spec);
  }
  const f = mockFetch(repoRoutes(fx("sd-rev"), fx("sd-tree")));
  assert.equal((await inspectCustomModel("owner/.hidden-ok", { fetch: f, info: INFO })).ok, true);
});
