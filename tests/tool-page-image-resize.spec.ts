import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-resize/ page resizes an uploaded image in-browser
// via ffmpeg (single-threaded @ffmpeg/core from jsDelivr — needs network).
test('image-resize page resizes an uploaded image', async ({ page }) => {
  await page.goto('/tools/image-resize/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-width', '1');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  // ffmpeg loads from CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
});
