import { test, expect } from './fixtures';
import path from 'node:path';

async function imageInfo(page: any, dataUrl: string) {
  return await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    return { width: img.naturalWidth, height: img.naturalHeight };
  }, dataUrl);
}

async function outputSrc(page: any) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

test('social-image-resize page makes an Instagram square image with real pixels', async ({ page }) => {
  await page.goto('/tools/social-image-resize/');
  await page.waitForSelector('#in-file', { state: 'attached' });

  await page.selectOption('#in-target', 'instagram-square');
  await page.selectOption('#in-fit', 'cover');
  await page.selectOption('#in-gravity', 'center');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1080);
  expect(info.height).toBe(1080);
});

test('social-image-resize deep-link supports target, fit, gravity and short hex background', async ({ page }) => {
  await page.goto('/tools/social-image-resize/?target=youtube-thumbnail&fit=contain&gravity=top&background=%23f00');
  await expect(page.locator('#in-target')).toHaveValue('youtube-thumbnail', { timeout: 15000 });
  await expect(page.locator('#in-fit')).toHaveValue('contain');
  await expect(page.locator('#in-gravity')).toHaveValue('top');
  await expect(page.locator('#in-background')).toHaveValue('#f00');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  const info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1280);
  expect(info.height).toBe(720);
});

test('social-image-resize page covers advertised non-default presets and stretch', async ({ page }) => {
  await page.goto('/tools/social-image-resize/');
  await page.waitForSelector('#in-file', { state: 'attached' });
  await page.selectOption('#in-target', 'linkedin-cover');
  await page.selectOption('#in-fit', 'stretch');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  let info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1584);
  expect(info.height).toBe(396);

  await page.selectOption('#in-target', 'tiktok-video');
  await page.selectOption('#in-fit', 'cover');
  await page.locator('#in-fit').dispatchEvent('change');
  info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1080);
  expect(info.height).toBe(1920);
});
