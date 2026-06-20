import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-mute/ page strips audio from an uploaded video
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output
// keeps the mp4 container, so the media src is a data:video/ URL.
test('video-mute page removes audio from an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-mute/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});
