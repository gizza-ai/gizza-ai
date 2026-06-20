import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-crop/ page crops an uploaded video in-browser via
// ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output keeps the mp4
// container, so the media src is a data:video/ URL.
test('video-crop page crops an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-crop/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-width', '64');
  await page.fill('#in-height', '64');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
});
