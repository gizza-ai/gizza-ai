import { test, expect } from './fixtures';
import path from 'node:path';

// The /tools/video-target-filesize-encoder/ page computes an H.264 bitrate from
// the clip DURATION so the re-encoded MP4 lands under a target MB budget. Its
// page/custom.js reads the duration (fast <video> path, ffmpeg probe fallback)
// then runs one ffmpeg.wasm encode (loaded from jsDelivr — needs network).
// Fixtures: tiny-av-128x128.mp4 (128x128, 1s, h264+aac) and clip-1s.webm
// (64x64, ~1s, vp8+vorbis, secondary input format → mp4).

const MP4 = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');
const WEBM = path.resolve(__dirname, 'fixtures/clip-1s.webm');
const MB = 1024 * 1024;

// Decode the produced MP4: real dimensions/duration (proves the encode happened)
// plus the exact encoded byte length (proves it fits the budget).
async function decodeVideo(page, src: string) {
  return page.evaluate(async (dataUrl: string) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('mp4 failed to decode')), { once: true });
    });
    const b64 = dataUrl.split(',')[1] || '';
    return { w: v.videoWidth, h: v.videoHeight, dur: v.duration, bytes: atob(b64).length };
  }, src);
}

test('encodes an mp4 under the target MB budget', async ({ page }) => {
  await page.goto('/tools/video-target-filesize-encoder/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_mb', '0.1');
  await page.setInputFiles('#in-file', MP4);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const r = await decodeVideo(page, src!);
  expect(r.w).toBe(128); // dimensions preserved (scale = keep)
  expect(r.h).toBe(128);
  expect(r.dur).toBeGreaterThan(0.5); // ~1s clip, real duration
  expect(r.bytes).toBeLessThanOrEqual(0.1 * MB); // actually under the budget
  expect(r.bytes).toBeGreaterThan(500); // and a real video, not empty
  await expect(page.locator('#tool-output')).toContainText('Done');
});

test('audio = none (non-default) still fits the budget', async ({ page }) => {
  await page.goto('/tools/video-target-filesize-encoder/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_mb', '0.1');
  await page.selectOption('#in-audio_kbps', 'none'); // non-default enum choice
  await page.setInputFiles('#in-file', MP4);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const r = await decodeVideo(page, src!);
  expect(r.w).toBe(128);
  expect(r.bytes).toBeLessThanOrEqual(0.1 * MB);
});

test('webm input (secondary format) is re-encoded to a playable mp4', async ({ page }) => {
  await page.goto('/tools/video-target-filesize-encoder/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_mb', '0.1');
  await page.setInputFiles('#in-file', WEBM);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/); // container switched webm → mp4

  const r = await decodeVideo(page, src!);
  expect(r.w).toBe(64); // 64x64 source preserved
  expect(r.bytes).toBeLessThanOrEqual(0.1 * MB);
});

test('deep-link pre-fills params and runs (?target_mb=&audio_kbps=none)', async ({ page }) => {
  await page.goto('/tools/video-target-filesize-encoder/?target_mb=0.15&audio_kbps=none');
  await page.waitForSelector('#in-file');
  // Scalar params are prefilled by the shared driver before custom.setup().
  await expect(page.locator('#in-target_mb')).toHaveValue('0.15');
  await page.setInputFiles('#in-file', MP4);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  const r = await decodeVideo(page, src!);
  expect(r.bytes).toBeLessThanOrEqual(0.15 * MB);
});
