import { test, expect } from './fixtures';
import path from 'node:path';
test('video-rotate page rotates an uploaded video', async ({ page }) => {
  await page.goto('/tools/video-rotate/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-rotate', '90');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  expect(await media.getAttribute('src')).toMatch(/^data:video\//);
});
