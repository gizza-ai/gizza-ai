import { test, expect, readResult } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// /tools/image-rounded-corners/ runs the block's core in a worker
// (`wasm-image` runtime) and shows the rounded PNG. Pixel expectations are
// pinned by the core test `page_spec_expectations_on_gray_fixture`.
async function pixels(page: Page, src: string | null, points: [number, number][]) {
  return page.evaluate(
    async ({ src, points }) => {
      const img = new Image();
      img.src = src!;
      await img.decode();
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      return {
        w: c.width,
        h: c.height,
        px: points.map(([x, y]) => Array.from(ctx.getImageData(x, y, 1, 1).data)),
      };
    },
    { src, points },
  );
}

test('image-rounded-corners makes the corners of an uploaded image transparent', async ({ page }) => {
  await page.goto('/tools/image-rounded-corners/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-radius', '16');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/gray-64x64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const src = await media.getAttribute('src');
  const { mime, bytes } = await readResult(page, src);
  expect(mime).toBe('image/png');
  expect(bytes.subarray(1, 4).toString('latin1')).toBe('PNG');
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'rounded.png');

  // 64x64 mid-gray, radius 16 px, all corners: corners fully transparent,
  // center and left-edge midpoint untouched, size unchanged (not cropped).
  const r = await pixels(page, src, [[0, 0], [63, 63], [32, 32], [0, 32]]);
  expect(r.w).toBe(64);
  expect(r.h).toBe(64);
  expect(r.px[0][3]).toBe(0);
  expect(r.px[1][3]).toBe(0);
  expect(r.px[2]).toEqual([128, 128, 128, 255]);
  expect(r.px[3]).toEqual([128, 128, 128, 255]);
});

test('image-rounded-corners fills only the top corners with a color', async ({ page }) => {
  await page.goto('/tools/image-rounded-corners/');
  await page.waitForSelector('#in-image');

  await page.fill('#in-radius', '16');
  await page.selectOption('#in-corners', 'top');
  await page.fill('#in-background', '#ff0000');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/gray-64x64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const r = await pixels(page, await media.getAttribute('src'), [[0, 0], [0, 63]]);
  expect(r.px[0]).toEqual([255, 0, 0, 255]); // top-left filled red, opaque
  expect(r.px[1]).toEqual([128, 128, 128, 255]); // bottom-left stays square
});

test('image-rounded-corners reports an undecodable file', async ({ page }) => {
  await page.goto('/tools/image-rounded-corners/');
  await page.setInputFiles('#in-image', {
    name: 'broken.png',
    mimeType: 'image/png',
    buffer: Buffer.from('not an image'),
  });
  await expect(page.locator('#tool-output')).toContainText('could not decode image', { timeout: 60_000 });
});

test('image-rounded-corners deep-link ?radius=50&unit=percent rounds into a circle', async ({ page }) => {
  await page.goto('/tools/image-rounded-corners/?radius=50&unit=percent');
  await expect(page.locator('#in-radius')).toHaveValue('50');
  await expect(page.locator('#in-unit')).toHaveValue('percent');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/gray-64x64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 60_000 });
  const src = await media.getAttribute('src');
  expect((await readResult(page, src)).mime).toBe('image/png');
  // 50% of the 64 px short side = 32 px radius: a full circle. (6, 6) lies
  // ~37 px from the center, outside it (a 16 px radius would have kept it);
  // (3, 32) is just inside the circle's anti-aliased left edge.
  const r = await pixels(page, src, [[0, 0], [6, 6], [32, 32], [3, 32]]);
  expect([r.w, r.h]).toEqual([64, 64]);
  expect(r.px[0][3]).toBe(0);
  expect(r.px[1][3]).toBe(0);
  expect(r.px[2]).toEqual([128, 128, 128, 255]);
  expect(r.px[3]).toEqual([128, 128, 128, 255]);
});
