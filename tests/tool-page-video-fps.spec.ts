import { test, expect } from './fixtures';
import path from 'node:path';

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

test('video-fps renders a real MP4 output at the default 30 fps setting', async ({ page }) => {
  await page.goto('/tools/video-fps/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fps')).toHaveAttribute('placeholder', '30');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
  expect(meta.d).toBeLessThan(2.0);
});

test('video-fps deep-link applies a non-default fps value and keeps webm output playable as mp4', async ({ page }) => {
  await page.goto('/tools/video-fps/?fps=24');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-fps')).toHaveValue('24');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/clip-1s.webm'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
  expect(meta.d).toBeGreaterThan(0.5);
  expect(meta.d).toBeLessThan(1.6);
});
