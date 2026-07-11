import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/mp4-to-mkv/ page rewraps an uploaded MP4 into a Matroska
// .mkv in-browser via ffmpeg (single-threaded @ffmpeg/core from jsDelivr — needs
// network). It's a lossless remux: `-i in.mp4 -map 0 -c copy out.mkv`, no params.
// The output is a data:video/x-matroska (video/*) URL.
//
// Tests assert REAL output correctness: they decode the produced video in a
// <video> element and check the preserved 128x128 dimensions + duration, so a
// transform that silently no-ops would FAIL. There are no query params, so no
// deep-link test applies.

const fixture = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

// Load the page's own wasm module and build the ffmpeg plan for a given input.
async function buildArgv(page, inName: string) {
  return await page.evaluate(async ({ inName }) => {
    const mod = await import('/tools/mp4-to-mkv/gizza_ai_mp4_to_mkv_web.js');
    await mod.default('/tools/mp4-to-mkv/gizza_ai_mp4_to_mkv_web_bg.wasm');
    return mod.build_argv(inName);
  }, { inName });
}

async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((res, rej) => {
      v.onloadedmetadata = () => res(null);
      v.onerror = () => rej(new Error('mp4-to-mkv output failed to decode'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

test('mp4-to-mkv wasm build_argv builds the exact lossless remux plan', async ({ page }) => {
  await page.goto('/tools/mp4-to-mkv/');
  await page.waitForSelector('#in-file');

  const plan = await buildArgv(page, 'in.mp4');
  expect(plan.out_name).toBe('out.mkv');
  // Lossless remux: select every stream (-map 0) and stream-copy (-c copy),
  // never re-encode. The argv is fully determined — assert it exactly.
  expect(plan.argv).toEqual(['-i', 'in.mp4', '-map', '0', '-c', 'copy', 'out.mkv']);
});

test('mp4-to-mkv page remuxes an MP4 to MKV losslessly', async ({ page }) => {
  await page.goto('/tools/mp4-to-mkv/');
  await page.waitForSelector('#in-file');

  // Upload a 128x128 H.264 MP4 fixture.
  await page.setInputFiles('#in-file', fixture);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  // Container changed .mp4 -> .mkv (Matroska); some builds label it generically.
  expect(src).toMatch(/^data:video\/(x-matroska|)/);

  // A remux must preserve the exact frame size and a real (>0) duration.
  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0);
});
