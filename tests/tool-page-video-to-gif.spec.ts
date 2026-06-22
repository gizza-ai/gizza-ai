import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-to-gif/ page converts a section of an uploaded
// video into an animated GIF in-browser via ffmpeg (@ffmpeg/core from jsDelivr —
// needs network). The output is a GIF rendered as an image, so the media src is
// a data:image/gif URL.
test('video-to-gif page converts an uploaded video to a GIF', async ({ page }) => {
  await page.goto('/tools/video-to-gif/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-start', '0');
  await page.fill('#in-duration', '1');
  await page.fill('#in-fps', '8');
  await page.fill('#in-width', '64');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/gif/);
});
