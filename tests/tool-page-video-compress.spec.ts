import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-compress/ page re-encodes an uploaded video
// in-browser via ffmpeg (single-threaded @ffmpeg/core from jsDelivr — needs
// network). A single-pass CRF re-encode keeps the mp4 container, so the output
// media src is a data:video/ URL.
test('video-compress page re-encodes an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-compress/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-crf', '30');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  // ffmpeg loads from CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
});
