import { test, expect, readResult } from './fixtures';
import path from 'node:path';

// The generated /tools/video-mute/ page strips audio from an uploaded video
// in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output
// keeps the mp4 container, so the media src is a video/ result.
test('video-mute page removes audio from an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-mute/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect((await readResult(page, await media.getAttribute('src'))).mime).toMatch(/^video\//);
});
