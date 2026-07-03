import { test, expect } from './fixtures';
import path from 'node:path';

test('video-trim page trims an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-trim/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-start', '0');
  await page.fill('#in-duration', '0.5');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});

// Family container-bug regression: trim is a lossless stream-copy (`-c copy`),
// which is only valid in the SOURCE's own container. A webm (VP8/Vorbis) copied
// into mp4 hard-fails, so the output must KEEP the webm container — and the page
// must resolve a `data:video/webm` URL that actually decodes in <video>.
test('video-trim page keeps a webm input as a playable webm', async ({ page }) => {
  await page.goto('/tools/video-trim/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-start', '0');
  await page.fill('#in-duration', '0.5');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/clip-1s.webm'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/webm/);
  // The webm must really decode: wait for the first frame and draw it to a canvas.
  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('webm failed to decode')), { once: true });
    });
    const c = document.createElement('canvas');
    c.width = v.videoWidth;
    c.height = v.videoHeight;
    c.getContext('2d')!.drawImage(v, 0, 0);
    return { w: v.videoWidth, h: v.videoHeight };
  }, src!);
  expect(frame.w).toBeGreaterThan(0);
  expect(frame.h).toBeGreaterThan(0);
});
