import { test, expect } from './fixtures';
import path from 'node:path';

// /tools/loop-video/ repeats a video in-browser via ffmpeg (@ffmpeg/core from
// jsDelivr — needs network). Output is a stream-copied video → data:video/ src.
test('loop-video page loops an uploaded video', async ({ page }) => {
  await page.goto('/tools/loop-video/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  await page.fill('#in-count', '3');
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});
