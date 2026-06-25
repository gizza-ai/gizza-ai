import { test, expect } from './fixtures';
import path from 'node:path';

test('image-crop page crops an uploaded image', async ({ page }) => {
  await page.goto('/tools/image-crop/');
  
  // Wait for the hidden image input to exist
  await page.waitForSelector('#in-image', { state: 'attached' });

  // Upload the 2x2 red test image first to make the inputs visible and interactive
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  // The preview image should load and make the workspace visible
  const previewImg = page.locator('#crop-preview-img');
  await expect(previewImg).toBeVisible();

  // Set the inputs to something small that fits within 2x2
  await page.fill('#in-x', '0');
  await page.fill('#in-y', '0');
  await page.fill('#in-width', '1');
  await page.fill('#in-height', '1');

  // Trigger change event to run crop
  await page.locator('#in-width').dispatchEvent('change');

  // ffmpeg loads from CDN on first run; allow generous time for processing.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
});
