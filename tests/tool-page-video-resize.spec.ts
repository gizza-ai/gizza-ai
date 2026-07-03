import { test, expect } from './fixtures';
import path from 'node:path';
test('video-resize page scales an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-resize/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '64');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});

// Family-wide container rule: a webm input can't keep its container through an
// H.264 re-encode (WebM only allows VP8/VP9/AV1 + Vorbis/Opus), so the output
// must switch to mp4 with the audio re-encoded to AAC — and actually decode.
test('video-resize page converts a webm input to a playable mp4', async ({ page }) => {
  await page.goto('/tools/video-resize/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '32');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/clip-1s.webm'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  // The mp4 must really decode: wait for the first frame and draw it to a canvas.
  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('mp4 failed to decode')), { once: true });
    });
    const c = document.createElement('canvas');
    c.width = v.videoWidth;
    c.height = v.videoHeight;
    c.getContext('2d')!.drawImage(v, 0, 0);
    return { w: v.videoWidth, h: v.videoHeight };
  }, src!);
  expect(frame.w).toBe(32);
  expect(frame.h).toBeGreaterThan(0);
});
