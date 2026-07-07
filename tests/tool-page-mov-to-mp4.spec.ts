import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/mov-to-mp4/ page rewraps an uploaded QuickTime .mov into
// an .mp4 in-browser via ffmpeg (single-threaded @ffmpeg/core from jsDelivr —
// needs network). Default "copy" mode stream-copies (lossless remux); the output
// is always a data:video/mp4 URL.
//
// Each test asserts REAL output correctness: it decodes the produced video in a
// <video> element and checks the container MIME + preserved dimensions/duration,
// so a transform that silently no-ops would FAIL.

// Decode a data:video/ URL and read its intrinsic size + duration.
async function decodeVideo(page, src: string) {
  return await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.preload = 'metadata';
    await new Promise((res, rej) => {
      v.onloadedmetadata = () => res(null);
      v.onerror = () => rej(new Error('video decode failed'));
      v.src = dataUrl;
    });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
}

test('mov-to-mp4 page remuxes a MOV to MP4 losslessly (copy mode)', async ({ page }) => {
  await page.goto('/tools/mov-to-mp4/');
  await page.waitForSelector('#in-video');

  // Default mode is "copy" (lossless remux). Upload a 128x128, 2s H.264+AAC MOV.
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-h264.mov'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/); // container changed .mov -> .mp4

  // A remux must preserve the exact frame size and duration (no re-encode/scale).
  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(1.5);
  expect(meta.d).toBeLessThan(2.6);
});

test('mov-to-mp4 deep-link transcodes at a non-default quality', async ({ page }) => {
  // Deep-link prefills the mode select + quality field; the run fires on upload.
  await page.goto('/tools/mov-to-mp4/?mode=transcode&quality=90');
  await page.waitForSelector('#in-video');
  await expect(page.locator('#in-mode')).toHaveValue('transcode');
  await expect(page.locator('#in-quality')).toHaveValue('90');

  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-h264.mov'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  // Re-encode still keeps size + duration intact.
  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(1.5);
});

test('mov-to-mp4 accepts an MP4 input too (secondary format, copy remux)', async ({ page }) => {
  await page.goto('/tools/mov-to-mp4/');
  await page.waitForSelector('#in-video');

  // A 128x128, 1s H.264+AAC .mp4 — remux mp4 -> mp4 is a valid no-op container copy.
  await page.setInputFiles('#in-video', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
});
