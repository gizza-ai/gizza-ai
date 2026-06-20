import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/change-speed/ page re-encodes an uploaded video in-browser
// via ffmpeg (@ffmpeg/core from jsDelivr — needs network). The fixture has no
// audio track, so the per-stream -filter:a is skipped (a good no-audio check).
test('change-speed page speeds up an uploaded video', async ({ page }) => {
  await page.goto('/tools/change-speed/');
  await page.waitForSelector('#in-file');

  await page.fill('#in-factor', '2');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});
