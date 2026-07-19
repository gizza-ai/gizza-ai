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

test('video-strip-metadata remuxes an MP4 without changing playable media', async ({ page }) => {
  await page.goto('/tools/video-strip-metadata/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-container')).toHaveValue('keep');
  await expect(page.locator('#in-chapters')).toHaveValue('remove');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
});

test('video-strip-metadata deep-link can keep chapters and force mp4 output', async ({ page }) => {
  await page.goto('/tools/video-strip-metadata/?container=mp4&chapters=keep');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-container')).toHaveValue('mp4');
  await expect(page.locator('#in-chapters')).toHaveValue('keep');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
}
);
