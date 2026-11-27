import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  TIMESTEP, SIGMA, VAE_SCALE, padTokens, noise, initLatents, scaleModelInput, stepEpsilon, latentsForVae, toImageData,
} from '../tools/generator/assets/runtime/sd-core.js';

const ref = JSON.parse(readFileSync(new URL('./fixtures/sd/reference.json', import.meta.url), 'utf8'));
const N = 4 * 64 * 64;

test('constants', () => {
  assert.equal(TIMESTEP, 999);
  assert.ok(Math.abs(SIGMA - 14.6146) < 1e-4);
  assert.equal(VAE_SCALE, 0.18215);
});

test('padTokens pads with 0 to 77 and matches the reference ids', () => {
  const out = padTokens(ref.token_ids_unpadded);
  assert.ok(out instanceof Int32Array);
  assert.equal(out.length, 77);
  assert.deepEqual(Array.from(out), ref.input_ids);
});

test('padTokens truncates long input to 77', () => {
  const out = padTokens(new Array(100).fill(5));
  assert.equal(out.length, 77);
});

test('noise is seeded, deterministic, ~N(0,1), and pinned to the spike generator', () => {
  const a = noise(ref.seed, N);
  assert.ok(a instanceof Float32Array);
  assert.equal(a.length, N);
  assert.deepEqual(Array.from(noise(ref.seed, N)), Array.from(a));
  // Pinned head of the spike's mulberry32 + Box-Muller stream (NOT torch.randn; the reference
  // run fed its own noise.bin to the pipeline, see noise_first16_torch_seed42).
  for (let i = 0; i < 16; i++) assert.ok(Math.abs(a[i] - ref.noise_first16_js_seed42[i]) < 1e-6, `i=${i}`);
  assert.notDeepEqual(Array.from(noise(7, 16)), Array.from(a.slice(0, 16)));
  let sum = 0, sq = 0;
  for (const v of a) { sum += v; sq += v * v; }
  assert.ok(Math.abs(sum / N) < 0.05);
  assert.ok(Math.abs(sq / N - 1) < 0.05);
});

test('scaleModelInput matches the reference scaled UNet input to 1e-6', () => {
  const s = scaleModelInput(Float32Array.from(ref.noise_first16_torch_seed42));
  assert.equal(s.length, 16);
  for (let i = 0; i < 16; i++) assert.ok(Math.abs(s[i] - ref.scaled_input_first16[i]) < 1e-6, `i=${i}`);
});

test('stepEpsilon: x0 = latents - sigma*eps', () => {
  const lat = Float32Array.from([1, 2, 3]);
  const eps = Float32Array.from([0.5, 0, -1]);
  const x0 = stepEpsilon(lat, eps);
  [1 - SIGMA * 0.5, 2, 3 + SIGMA].forEach((v, i) => assert.ok(Math.abs(x0[i] - v) < 1e-5));
});

test('latentsForVae divides by VAE_SCALE', () => {
  const v = latentsForVae(Float32Array.from([0.18215, -0.3643]));
  assert.ok(Math.abs(v[0] - 1) < 1e-6 && Math.abs(v[1] + 2) < 1e-6);
});

test('toImageData maps [-1,1] to [0,255], clamps, and is opaque RGBA', () => {
  // 2x1 image, CHW: R=[-1,1] G=[0,5] B=[-5,0]
  const out = toImageData(Float32Array.from([-1, 1, 0, 5, -5, 0]), 2, 1);
  assert.ok(out instanceof Uint8ClampedArray);
  assert.deepEqual(Array.from(out), [0, 128, 0, 255, 255, 255, 128, 255]);
});

test('initLatents scales raw noise by SIGMA: the latents stepEpsilon expects', () => {
  const raw = Float32Array.from([1, -2, 0.5]);
  const lat = initLatents(raw);
  assert.ok(lat instanceof Float32Array);
  assert.deepEqual(Array.from(lat), Array.from(raw, (v) => Math.fround(v * SIGMA)));
  // Zero predicted noise leaves x0 at the noised latents themselves.
  assert.deepEqual(Array.from(stepEpsilon(lat, new Float32Array(3))), Array.from(lat));
});
