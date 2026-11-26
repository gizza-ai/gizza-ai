import { test, expect, readResult } from './fixtures';

// /tools/rotate-image/ runs the block's core in a worker (`wasm-image`
// runtime) and shows the rotated PNG.

// A 4x2 PNG: red at the top-left pixel, blue everywhere else. Generated
// in-page so the fixture is non-square with one trackable pixel.
async function makeFixture(page: import('@playwright/test').Page): Promise<Buffer> {
  const b64 = await page.evaluate(() => {
    const c = document.createElement('canvas');
    c.width = 4;
    c.height = 2;
    const ctx = c.getContext('2d')!;
    ctx.fillStyle = '#0000ff';
    ctx.fillRect(0, 0, 4, 2);
    ctx.fillStyle = '#ff0000';
    ctx.fillRect(0, 0, 1, 1);
    return c.toDataURL('image/png').split(',')[1];
  });
  return Buffer.from(b64, 'base64');
}

test('rotate-image turns an uploaded image 90° clockwise', async ({ page }) => {
  await page.goto('/tools/rotate-image/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-angle', '90');
  await page.setInputFiles('#in-image', {
    name: 'wide.png',
    mimeType: 'image/png',
    buffer: await makeFixture(page),
  });

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const { mime, bytes } = await readResult(page, await media.getAttribute('src'));
  expect(mime).toBe('image/png');
  expect(bytes.subarray(1, 4).toString('latin1')).toBe('PNG');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'rotated.png');

  // Decode the result in-page: dimensions swapped, and 90° clockwise maps
  // source (x, y) -> (h-1-y, x), so the red top-left pixel lands top-right.
  const out = await page.evaluate(async (src) => {
    const img = new Image();
    img.src = src!;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const px = (x: number, y: number) => Array.from(ctx.getImageData(x, y, 1, 1).data);
    return { w: c.width, h: c.height, topRight: px(1, 0), topLeft: px(0, 0) };
  }, await media.getAttribute('src'));
  expect(out.w).toBe(2);
  expect(out.h).toBe(4);
  expect(out.topRight).toEqual([255, 0, 0, 255]);
  expect(out.topLeft).toEqual([0, 0, 255, 255]);
});

test('rotate-image reports an undecodable file', async ({ page }) => {
  await page.goto('/tools/rotate-image/');
  await page.setInputFiles('#in-image', {
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image'),
  });
  await expect(page.locator('#tool-output')).toContainText('could not decode image', { timeout: 60_000 });
});
