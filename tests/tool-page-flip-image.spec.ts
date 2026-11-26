import { test, expect, readResult } from './fixtures';
import path from 'node:path';

// /tools/flip-image/ runs the block's core in a worker (`wasm-image` runtime)
// and shows the flipped PNG.
test('flip-image mirrors an uploaded image horizontally', async ({ page }) => {
  await page.goto('/tools/flip-image/');
  await page.waitForSelector('#in-image');

  await page.selectOption('#in-direction', 'horizontal');
  // Quadrants fixture: red top-left, green top-right, blue bottom-left, white bottom-right.
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/quadrants-64x64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const { mime, bytes } = await readResult(page, await media.getAttribute('src'));
  expect(mime).toBe('image/png');
  expect(bytes.subarray(1, 4).toString('latin1')).toBe('PNG');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'flipped.png');

  // The worker really flipped it: decode the result in-page.
  const px = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src!;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const at = (x: number, y: number) => Array.from(ctx.getImageData(x, y, 1, 1).data);
    return { w: c.width, h: c.height, tl: at(0, 0), tr: at(63, 0), bl: at(0, 63) };
  }, await media.getAttribute('src'));
  expect(px.w).toBe(64);
  expect(px.h).toBe(64);
  // Left ↔ right swapped: top-left now has the original top-right green, and
  // vice versa; bottom-left now has the original bottom-right white.
  expect(px.tl).toEqual([0, 255, 0, 255]);
  expect(px.tr).toEqual([255, 0, 0, 255]);
  expect(px.bl).toEqual([255, 255, 255, 255]);
});

test('flip-image reports an undecodable file', async ({ page }) => {
  await page.goto('/tools/flip-image/');
  await page.setInputFiles('#in-image', {
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image'),
  });
  await expect(page.locator('#tool-output')).toContainText('could not decode image', { timeout: 60_000 });
});
