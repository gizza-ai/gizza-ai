import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-compress/ page re-encodes an uploaded image at a
// chosen quality in-browser via ffmpeg (single-threaded @ffmpeg/core from
// jsDelivr — needs network), keeping the same format.
test('image-compress page compresses an uploaded image', async ({ page }) => {
  await page.goto('/tools/image-compress/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-quality', '40');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  // ffmpeg loads from CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  // Same format in → out: a PNG upload yields a PNG data URL.
  expect(src).toMatch(/^data:image\/png/);
});
