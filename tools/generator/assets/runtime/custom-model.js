// Custom model check for the image-model picker. Given a Hugging Face repo id
// it reads metadata only (model info + file tree), never weights, and returns
// a registry entry the picker can use. Every request goes to the Hugging Face
// API through the injected `fetch`; no /resolve/ URL is ever requested.

import { JANUS_DEVICES, JANUS_DTYPES, JANUS_NEEDS_F16 } from "./janus-builds.js";

const HF = "https://huggingface.co";
const SPEC = /^([A-Za-z0-9][\w.-]*)\/([\w.-]+?)(?:@([^\s@]+))?$/;
const DOTS_ONLY = /^\.+$/;
const COMMIT_SHA = /^[0-9a-f]{40}$/;
const MAX_TREE_PAGES = 100;

const UNSUPPORTED =
  "This repo isn't a supported format. gizza runs Janus-Pro web ONNX repos and gizza's own SD-Turbo web conversions.";
const NEEDS_LOGIN = "needs a Hugging Face login, which gizza doesn't support";
// Anonymous API calls get 401 for a repo that doesn't exist too, so a typo
// can't be told apart from a private or gated repo.
const NOT_FOUND_OR_LOCKED = `not found, or private/gated (${NEEDS_LOGIN})`;

const JANUS_COMPONENTS = Object.keys(JANUS_DEVICES);
// The onnx file suffix of each dtype.
const DTYPE_SUFFIX = { fp32: "", fp16: "_fp16", q4: "_q4", q4f16: "_q4f16", q8: "_quantized" };
const SD_VARIANTS = { nof16q8: false, fp16: true };
const SD_COMPONENTS = ["text_encoder", "unet", "vae_decoder"];

class Refusal extends Error {}

async function getOk(fetchFn, url) {
  let res;
  try {
    res = await fetchFn(url);
  } catch {
    throw new Refusal("couldn't reach Hugging Face");
  }
  if (res.ok) return res;
  if (res.status === 404) throw new Refusal("repo or revision not found on Hugging Face");
  if (res.status === 429) throw new Refusal("Hugging Face is rate-limiting; try again in a minute");
  if (res.status === 401 || res.status === 403) throw new Refusal(NOT_FOUND_OR_LOCKED);
  throw new Refusal(`Hugging Face returned an error (HTTP ${res.status})`);
}

// The next tree page, which must stay on the Hugging Face API.
function nextLink(res) {
  const header = res.headers.get("link");
  if (!header) return null;
  for (const part of header.split(",")) {
    const m = part.match(/<([^>]+)>\s*;\s*rel="?next"?/);
    if (!m) continue;
    let next;
    try {
      next = new URL(m[1]);
    } catch {
      next = null;
    }
    if (!next || next.origin !== HF) throw new Refusal("Hugging Face returned an unexpected next-page link");
    return next.href;
  }
  return null;
}

async function readTree(fetchFn, repo, sha) {
  const files = new Map(); // path -> bytes
  let url = `${HF}/api/models/${repo}/tree/${sha}?recursive=true`;
  for (let page = 0; url && page < MAX_TREE_PAGES; page++) {
    const res = await getOk(fetchFn, url);
    for (const item of await res.json()) {
      if (item.type === "file") files.set(item.path, item.lfs?.size ?? item.size ?? 0);
    }
    url = nextLink(res);
  }
  return files;
}

const toMb = (bytes) => Math.round(bytes / 1e6);

// Sum of the given paths, or null when any required path is missing. Paths
// ending in `?` are optional (the external-data file of a small graph).
function sumFiles(files, paths) {
  let total = 0;
  for (const p of paths) {
    const optional = p.endsWith("?");
    const key = optional ? p.slice(0, -1) : p;
    if (!files.has(key)) {
      if (optional) continue;
      return null;
    }
    total += files.get(key);
  }
  return total;
}

function janusVariants(files) {
  const out = {};
  for (const [name, dtypes] of Object.entries(JANUS_DTYPES)) {
    const paths = JANUS_COMPONENTS.flatMap((c) => {
      const base = `onnx/${c}${DTYPE_SUFFIX[dtypes[c]]}.onnx`;
      return [base, `${base}_data?`];
    });
    const bytes = sumFiles(files, paths);
    if (bytes !== null) {
      out[name] = { requires: { shaderF16: JANUS_NEEDS_F16[name] }, download_mb: toMb(bytes), verified: false };
    }
  }
  return out;
}

// The SD worker sizes its downloads from manifest.json, so the check needs it.
function sdVariants(files) {
  const shared = sumFiles(files, ["tokenizer/tokenizer.json", "manifest.json"]);
  const out = {};
  if (shared === null) return out;
  for (const [name, shaderF16] of Object.entries(SD_VARIANTS)) {
    const bytes = sumFiles(
      files,
      SD_COMPONENTS.flatMap((c) => [`${name}/${c}/model.onnx`, `${name}/${c}/model.onnx.data`]),
    );
    if (bytes !== null) {
      out[name] = { requires: { shaderF16 }, download_mb: toMb(shared + bytes), verified: false };
    }
  }
  return out;
}

function licenseOf(repo, cardData) {
  const raw = cardData?.license;
  const names = (Array.isArray(raw) ? raw : [raw]).filter((n) => typeof n === "string" && n.trim());
  if (names.length === 0) {
    return {
      name: "no license stated",
      url: `${HF}/${repo}`,
      summary: "The repo states no license. Check the model card before using results.",
    };
  }
  const name = names.join(", ");
  return {
    name,
    url: `${HF}/${repo}`,
    summary: `License declared by the repo: ${name}. Check the model card before using results.`,
  };
}

// spec: "owner/name" or "owner/name@rev". `info` is the WebGPU adapter info
// ({ webgpu, shaderF16, maxBufferSize }); each variant already carries its
// own size and requirement, so the picker resolves the variant from it.
export async function inspectCustomModel(spec, { fetch: fetchFn } = {}) {
  const m = SPEC.exec(String(spec).trim());
  if (!m || DOTS_ONLY.test(m[2])) return { ok: false, reason: "enter a Hugging Face repo as owner/name" };
  const repo = `${m[1]}/${m[2]}`;
  const rev = m[3] || "main";
  try {
    const metaRes = await getOk(fetchFn, `${HF}/api/models/${repo}/revision/${encodeURIComponent(rev)}`);
    const meta = await metaRes.json();
    if (meta.gated || meta.private) return { ok: false, reason: NEEDS_LOGIN };
    const sha = meta.sha;
    if (typeof sha !== "string" || !COMMIT_SHA.test(sha)) {
      throw new Refusal("Hugging Face didn't return a commit for this revision");
    }
    const files = await readTree(fetchFn, repo, sha);

    let engine = null;
    let variants = {};
    let output = null;
    if (meta.config?.model_type === "multi_modality") {
      variants = janusVariants(files);
      engine = "janus";
      output = { w: 384, h: 384 };
    } else {
      variants = sdVariants(files);
      engine = "sd-turbo";
      output = { w: 512, h: 512 };
    }
    if (Object.keys(variants).length === 0) return { ok: false, reason: UNSUPPORTED };

    return {
      ok: true,
      recognized: "format",
      entry: {
        id: `custom:${repo}@${sha}`,
        label: m[2],
        tagline: `Custom model from Hugging Face (${repo})`,
        engine,
        repo,
        revision: sha,
        output,
        license: licenseOf(repo, meta.cardData),
        variants,
      },
    };
  } catch (e) {
    if (e instanceof Refusal) return { ok: false, reason: e.message };
    throw e;
  }
}
