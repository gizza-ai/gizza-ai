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

test('video-remove-embedded-subs remuxes an MP4 and keeps the playable media', async ({ page }) => {
  await page.goto('/tools/video-remove-embedded-subs/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-container')).toHaveValue('keep');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-av-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  // keep container preserves the input extension → mp4 in / mp4 out.
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBe(128);
  expect(meta.h).toBe(128);
  expect(meta.d).toBeGreaterThan(0.5);
});

test('video-remove-embedded-subs deep-link forces mp4 output from an mkv source', async ({ page }) => {
  await page.goto('/tools/video-remove-embedded-subs/?container=mp4');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-container')).toHaveValue('mp4');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-h264.mkv'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  // container=mp4 remuxes the h264 mkv into an mp4 without re-encoding.
  expect(src).toMatch(/^data:video\/mp4/);

  const meta = await decodeVideo(page, src!);
  expect(meta.w).toBeGreaterThan(0);
  expect(meta.h).toBeGreaterThan(0);
});
