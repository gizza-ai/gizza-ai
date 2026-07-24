import { test, expect } from './fixtures';
import path from 'node:path';

const fixture = path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4');

async function expectBakedVideoDataUrl(page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);
  const frame = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('video-bake-rotation output failed to decode')), { once: true });
    });
    return { w: v.videoWidth, h: v.videoHeight, duration: v.duration };
  }, src!);
  expect(frame.w).toBe(128);
  expect(frame.h).toBe(128);
  expect(frame.duration).toBeGreaterThan(0);
}

test('video-bake-rotation page bakes/normalizes a video', async ({ page }) => {
  await page.goto('/tools/video-bake-rotation/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);
  await expectBakedVideoDataUrl(page);
});

test('video-bake-rotation page ignores unrelated query params and still runs', async ({ page }) => {
  await page.goto('/tools/video-bake-rotation/?utm_source=test');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', fixture);
  await expectBakedVideoDataUrl(page);
});
