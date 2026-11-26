import { test, expect, readResult } from './fixtures';
import * as path from 'path';

test('image-grayscale page', async ({ page }) => {
  await page.goto('/tools/image-grayscale/');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90000 });
  expect((await readResult(page, await media.getAttribute('src'))).mime).toMatch(/^image\//);
});
