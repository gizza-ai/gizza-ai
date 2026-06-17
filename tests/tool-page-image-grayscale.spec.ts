import { test, expect } from './fixtures';
import * as path from 'path';

test('image-grayscale page', async ({ page }) => {
  await page.goto('/tools/image-grayscale/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  expect(await media.getAttribute('src')).toMatch(/^data:image\//);
});
