import { test, expect, readResult } from './fixtures';
import path from 'node:path';

// The generated /tools/gif-to-mp4/ page converts an uploaded GIF in-browser via
// ffmpeg (@ffmpeg/core from jsDelivr — needs network). Output is an mp4, so the
// media src is a video/ result.
test('gif-to-mp4 page converts an uploaded GIF', async ({ page }) => {
  await page.goto('/tools/gif-to-mp4/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny.gif'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect((await readResult(page, await media.getAttribute('src'))).mime).toMatch(/^video\//);
});
