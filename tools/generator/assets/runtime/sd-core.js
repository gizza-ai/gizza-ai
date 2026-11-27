// SD-Turbo 1-step pipeline math (pure: no onnxruntime, no tokenizer). The model worker wires ORT + the tokenizer
// around these helpers. Mirrors diffusers StableDiffusionPipeline + EulerDiscreteScheduler(timestep_spacing="trailing",
// beta_schedule="scaled_linear", 0.00085..0.012, 1000 steps, prediction_type="epsilon"), guidance 0.

const f = Math.fround;

export const SEQ_LEN = 77;
export const TIMESTEP = 999;
export const VAE_SCALE = 0.18215;

// sigma at the single trailing timestep (999), computed in float32 like diffusers' float32 betas.
function sigmaAt(t, numTrain = 1000, betaStart = 0.00085, betaEnd = 0.012) {
  const s0 = Math.sqrt(betaStart), s1 = Math.sqrt(betaEnd);
  let acc = 1;
  let alphaT = 1;
  for (let i = 0; i <= t; i++) {
    const b = f(f(s0 + (s1 - s0) * i / (numTrain - 1)) ** 2);
    acc = f(acc * f(1 - b));
    alphaT = acc;
  }
  return f(Math.sqrt(f(f(1 - alphaT) / alphaT)));
}

export const SIGMA = sigmaAt(TIMESTEP); // 14.6146...; also init_noise_sigma for trailing spacing

// Pads with 0 ("!", the SD 2.x tokenizer pad) / truncates to 77 like CLIPTokenizer(padding="max_length", truncation=True).
export function padTokens(ids) {
  const out = new Int32Array(SEQ_LEN);
  const n = Math.min(ids.length, SEQ_LEN);
  for (let i = 0; i < n; i++) out[i] = Number(ids[i]);
  return out;
}

// Seeded RNG (mulberry32) + Box-Muller -> N(0,1) Float32Array of length n.
export function noise(seed, n) {
  let a = seed >>> 0;
  const u = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const out = new Float32Array(n);
  for (let i = 0; i < n; i += 2) {
    let u1 = u(); if (u1 < 1e-12) u1 = 1e-12;
    const u2 = u();
    const r = Math.sqrt(-2 * Math.log(u1));
    out[i] = r * Math.cos(2 * Math.PI * u2);
    if (i + 1 < n) out[i + 1] = r * Math.sin(2 * Math.PI * u2);
  }
  return out;
}

// Raw N(0,1) noise -> the noised latents (noise * init_noise_sigma) that stepEpsilon takes.
export function initLatents(raw) {
  const out = new Float32Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw[i] * SIGMA;
  return out;
}

// Raw N(0,1) noise -> scaled UNet input: (noise * init_noise_sigma) / sqrt(sigma^2 + 1).
export function scaleModelInput(latents) {
  const k = SIGMA / Math.sqrt(SIGMA * SIGMA + 1);
  const out = new Float32Array(latents.length);
  for (let i = 0; i < latents.length; i++) out[i] = latents[i] * k;
  return out;
}

// Euler step to sigma_next = 0 with epsilon prediction: prev = x0 = latents - sigma * eps.
// `latents` is the noised latent (noise * SIGMA), not the scaled UNet input.
export function stepEpsilon(latents, eps) {
  const out = new Float32Array(latents.length);
  for (let i = 0; i < latents.length; i++) out[i] = latents[i] - SIGMA * eps[i];
  return out;
}

export function latentsForVae(x0) {
  const out = new Float32Array(x0.length);
  for (let i = 0; i < x0.length; i++) out[i] = x0[i] / VAE_SCALE;
  return out;
}

// CHW float (-1..1) -> opaque RGBA bytes, as diffusers: (x/2+0.5).clamp(0,1) * 255, rounded.
export function toImageData(vaeOut, w, h) {
  const out = new Uint8ClampedArray(h * w * 4);
  const plane = h * w;
  for (let p = 0; p < plane; p++) {
    for (let c = 0; c < 3; c++) {
      const v = Math.min(1, Math.max(0, vaeOut[c * plane + p] / 2 + 0.5));
      out[p * 4 + c] = Math.round(v * 255);
    }
    out[p * 4 + 3] = 255;
  }
  return out;
}
