import { test, expect, readResult } from './fixtures';
import path from 'node:path';

// /tools/image-border-frame/ runs the block's core in a worker (`wasm-image`
// runtime) and shows the framed PNG. Expectations mirror the core test
// `apply_returns_framed_png_and_name`: red 2x2 + 3px #0000ff → 8x8, corner
// blue, original red pixels at (3..=4, 3..=4).
test('image-border-frame adds a colored border to an uploaded image', async ({ page }) => {
  await page.goto('/tools/image-border-frame/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-thickness', '3');
  await page.fill('#in-color', '#0000ff');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const { mime, bytes } = await readResult(page, await media.getAttribute('src'));
  expect(mime).toBe('image/png');
  expect(bytes.subarray(1, 4).toString('latin1')).toBe('PNG');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'framed.png');

  // The worker really framed the image: decode the result in-page.
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
    return { w: c.width, h: c.height, corner: at(0, 0), far: at(7, 7), edge: at(2, 3), center: at(3, 3) };
  }, await media.getAttribute('src'));
  // 2 + 2*3 = 8 in each dimension.
  expect(px.w).toBe(8);
  expect(px.h).toBe(8);
  expect(px.corner).toEqual([0, 0, 255, 255]);
  expect(px.far).toEqual([0, 0, 255, 255]);
  expect(px.edge).toEqual([0, 0, 255, 255]);
  expect(px.center).toEqual([255, 0, 0, 255]);
});

test('image-border-frame reports an undecodable file', async ({ page }) => {
  await page.goto('/tools/image-border-frame/');
  await page.setInputFiles('#in-image', {
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image'),
  });
  await expect(page.locator('#tool-output')).toContainText('could not decode image', { timeout: 60_000 });
});

test('image-border-frame deep-link ?thickness=2&color=%230f0 frames with a short-hex color', async ({ page }) => {
  await page.goto('/tools/image-border-frame/?thickness=2&color=%230f0');
  await expect(page.locator('#in-thickness')).toHaveValue('2');
  await expect(page.locator('#in-color')).toHaveValue('#0f0');
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
    const at = (x: number, y: number) => Array.from(ctx.getImageData(x, y, 1, 1).data);
    return { w: c.width, h: c.height, corner: at(0, 0), edge: at(1, 2), center: at(2, 2) };
  }, src);
  // 2 + 2*2 = 6 in each dimension; #0f0 expands to opaque green.
  expect([px.w, px.h]).toEqual([6, 6]);
  expect(px.corner).toEqual([0, 255, 0, 255]);
  expect(px.edge).toEqual([0, 255, 0, 255]);
  expect(px.center).toEqual([255, 0, 0, 255]);
});
