// Custom model check for the image-model picker. Given a Hugging Face repo id
// it reads metadata only (model info + file tree), never weights, and returns
// a registry entry the picker can use. Every request goes to the Hugging Face
// API through the injected `fetch`; no /resolve/ URL is ever requested.

const HF = "https://huggingface.co";
const SPEC = /^([A-Za-z0-9][\w.-]*)\/([\w.-]+?)(?:@([^\s@]+))?$/;
const MAX_TREE_PAGES = 100;

const UNSUPPORTED =
  "This repo isn't a supported format. gizza runs Janus-Pro web ONNX repos and gizza's own SD-Turbo web conversions.";
const NEEDS_LOGIN = "needs a Hugging Face login, which gizza doesn't support";

const JANUS_COMPONENTS = [
  "prepare_inputs_embeds",
  "language_model",
  "lm_head",
  "gen_head",
  "gen_img_embeds",
  "image_decode",
];
// Mirrors JANUS_DTYPES in model-worker.js; suffix is the onnx file suffix.
const DTYPE_SUFFIX = { fp32: "", fp16: "_fp16", q4: "_q4", q4f16: "_q4f16", q8: "_quantized" };
const JANUS_VARIANTS = {
  f16: {
    shaderF16: true,
    dtypes: {
      prepare_inputs_embeds: "q4",
      language_model: "q4f16",
      lm_head: "fp16",
      gen_head: "fp16",
      gen_img_embeds: "fp16",
      image_decode: "fp32",
    },
  },
  f32: {
    shaderF16: false,
    dtypes: {
      prepare_inputs_embeds: "q8",
      language_model: "q4",
      lm_head: "q4",
      gen_head: "q4",
      gen_img_embeds: "fp32",
      image_decode: "fp32",
    },
  },
};
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
  if (res.status === 401 || res.status === 403) throw new Refusal(NEEDS_LOGIN);
  throw new Refusal(`Hugging Face returned an error (HTTP ${res.status})`);
}

function nextLink(res) {
  const header = res.headers.get("link");
  if (!header) return null;
  for (const part of header.split(",")) {
    const m = part.match(/<([^>]+)>\s*;\s*rel="?next"?/);
    if (m) return m[1];
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
  for (const [name, v] of Object.entries(JANUS_VARIANTS)) {
    const paths = JANUS_COMPONENTS.flatMap((c) => {
      const base = `onnx/${c}${DTYPE_SUFFIX[v.dtypes[c]]}.onnx`;
      return [base, `${base}_data?`];
    });
    const bytes = sumFiles(files, paths);
    if (bytes !== null) {
      out[name] = { requires: { shaderF16: v.shaderF16 }, download_mb: toMb(bytes), verified: false };
    }
  }
  return out;
}

function sdVariants(files) {
  const tokenizer = sumFiles(files, ["tokenizer/tokenizer.json"]);
  const out = {};
  if (tokenizer === null) return out;
  for (const [name, shaderF16] of Object.entries(SD_VARIANTS)) {
    const bytes = sumFiles(
      files,
      SD_COMPONENTS.flatMap((c) => [`${name}/${c}/model.onnx`, `${name}/${c}/model.onnx.data`]),
    );
    if (bytes !== null) {
      out[name] = { requires: { shaderF16 }, download_mb: toMb(tokenizer + bytes), verified: false };
    }
  }
  return out;
}

function licenseOf(repo, cardData) {
  const raw = cardData?.license;
  const name = Array.isArray(raw) ? raw.join(", ") : raw || "unspecified";
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
  if (!m) return { ok: false, reason: "enter a Hugging Face repo as owner/name" };
  const repo = `${m[1]}/${m[2]}`;
  const rev = m[3] || "main";
  try {
    const metaRes = await getOk(fetchFn, `${HF}/api/models/${repo}/revision/${encodeURIComponent(rev)}`);
    const meta = await metaRes.json();
    if (meta.gated || meta.private) return { ok: false, reason: NEEDS_LOGIN };
    const sha = meta.sha;
    if (!sha) throw new Refusal("Hugging Face didn't return a commit for this revision");
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
