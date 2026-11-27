import { test, expect, readResult } from './fixtures';
import path from 'node:path';

// /tools/image-brightness-contrast/ runs the block's core in a worker
// (`wasm-image` runtime) and shows the adjusted PNG.
test('image-brightness-contrast brightens an uploaded image', async ({ page }) => {
  await page.goto('/tools/image-brightness-contrast/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-brightness', '60');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const { mime, bytes } = await readResult(page, await media.getAttribute('src'));
  expect(mime).toBe('image/png');
  expect(bytes.subarray(1, 4).toString('latin1')).toBe('PNG');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'adjusted.png');

  // The worker really applied the adjustment: decode the result in-page.
  const px = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src!;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    return { w: c.width, h: c.height, rgba: Array.from(ctx.getImageData(0, 0, 1, 1).data) };
  }, await media.getAttribute('src'));
  expect(px.w).toBe(2);
  expect(px.h).toBe(2);
  // red (255,0,0) + 60 brightness → green/blue lifted to 60, red stays 255.
  expect(px.rgba).toEqual([255, 60, 60, 255]);
});

test('image-brightness-contrast reports an undecodable file', async ({ page }) => {
  await page.goto('/tools/image-brightness-contrast/');
  await page.setInputFiles('#in-image', {
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image'),
  });
  await expect(page.locator('#tool-output')).toContainText('could not decode image', { timeout: 60_000 });
});

test('image-brightness-contrast deep-link ?brightness=-40 darkens the image', async ({ page }) => {
  await page.goto('/tools/image-brightness-contrast/?brightness=-40');
  await expect(page.locator('#in-brightness')).toHaveValue('-40');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const src = await media.getAttribute('src');
  expect((await readResult(page, src)).mime).toBe('image/png');
  const px = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src!;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    return { w: c.width, h: c.height, rgba: Array.from(ctx.getImageData(1, 1, 1, 1).data) };
  }, src);
  expect([px.w, px.h]).toEqual([2, 2]);
  // red (255,0,0) - 40 brightness → red channel 215; green/blue floor at 0.
  expect(px.rgba).toEqual([215, 0, 0, 255]);
});
