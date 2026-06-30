import { test, expect } from './fixtures';
import path from 'node:path';

test('photo-filter-presets page applies default sepia filter', async ({ page }) => {
  await page.goto('/tools/photo-filter-presets/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
});

test('photo-filter-presets page applies noir preset', async ({ page }) => {
  await page.goto('/tools/photo-filter-presets/');
  await page.selectOption('#in-preset', 'noir');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
});
