import { test, expect } from './fixtures';
import path from 'node:path';

async function imageInfo(page: any, dataUrl: string) {
  return await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src;
    await img.decode();
    const canvas = document.createElement('canvas');
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const center = Array.from(ctx.getImageData(Math.floor(canvas.width / 2), Math.floor(canvas.height / 2), 1, 1).data);
    const corner = Array.from(ctx.getImageData(0, 0, 1, 1).data);
    return { width: img.naturalWidth, height: img.naturalHeight, center, corner };
  }, dataUrl);
}

async function outputSrc(page: any) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

test('pin-image-resizer page makes a 1000x1500 standard pin with real pixels', async ({ page }) => {
  await page.goto('/tools/pin-image-resizer/');
  await page.waitForSelector('#in-file', { state: 'attached' });

  await page.selectOption('#in-preset', 'standard');
  await page.selectOption('#in-fit', 'cover');
  await page.selectOption('#in-gravity', 'center');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1000);
  expect(info.height).toBe(1500);
  expect(info.center[0]).toBeGreaterThan(200);
  expect(info.center[1]).toBeLessThan(40);
  expect(info.center[2]).toBeLessThan(40);
});

test('pin-image-resizer supports preset and fit controls plus a query-param deep-link', async ({ page }) => {
  await page.goto('/tools/pin-image-resizer/?preset=story&fit=contain&gravity=top&background=%23f00');
  await expect(page.locator('#in-preset')).toHaveValue('story');
  await expect(page.locator('#in-fit')).toHaveValue('contain');
  await expect(page.locator('#in-gravity')).toHaveValue('top');
  await expect(page.locator('#in-background')).toHaveValue('#f00');

  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/red-2x2.png'));
  let info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1080);
  expect(info.height).toBe(1920);

  await page.selectOption('#in-preset', 'square');
  await page.selectOption('#in-fit', 'stretch');
  await page.locator('#in-fit').dispatchEvent('change');
  info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1000);
  expect(info.height).toBe(1000);

  await page.selectOption('#in-preset', 'tall');
  await page.selectOption('#in-fit', 'contain');
  await page.fill('#in-background', '#ffffff');
  await page.locator('#in-background').dispatchEvent('change');
  info = await imageInfo(page, await outputSrc(page));
  expect(info.width).toBe(1000);
  expect(info.height).toBe(2100);
});
