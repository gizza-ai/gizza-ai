import { test, expect } from './fixtures';
import path from 'node:path';

const WHITE = path.resolve(__dirname, 'fixtures/white-64x64.png');
const QUADRANTS = path.resolve(__dirname, 'fixtures/quadrants-64x64.png');
const FF_TIMEOUT = 90_000;

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: FF_TIMEOUT });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

async function samplePixels(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = src; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const px = (x: number, y: number) => {
      const d = ctx.getImageData(x, y, 1, 1).data;
      return { r: d[0], g: d[1], b: d[2] };
    };
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      topLeft: px(0, 0),
      center: px(Math.floor(img.naturalWidth / 2), Math.floor(img.naturalHeight / 2)),
      bottomRight: px(img.naturalWidth - 1, img.naturalHeight - 1),
    };
  }, dataUrl);
}

async function setInsetDistort(page: import('@playwright/test').Page) {
  await page.selectOption('#in-mode', 'distort');
  await page.fill('#in-tl_x', '12');
  await page.fill('#in-tl_y', '12');
  await page.fill('#in-tr_x', '56');
  await page.fill('#in-tr_y', '12');
  await page.fill('#in-bl_x', '12');
  await page.fill('#in-bl_y', '56');
  await page.fill('#in-br_x', '56');
  await page.fill('#in-br_y', '56');
}

test('image-perspective-warp page distorts a quadrant image into an inset frame', async ({ page }) => {
  await page.goto('/tools/image-perspective-warp/');
  await setInsetDistort(page);
  await page.setInputFiles('#in-image', QUADRANTS);

  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // A real perspective distort to the inset preset pulls the top-left red
  // quadrant toward the center. A no-op would leave the center white because
  // the fixture's bottom-right quadrant starts at (32,32).
  expect(p.center.r).toBeGreaterThan(220);
  expect(p.center.g).toBeLessThan(80);
  expect(p.center.b).toBeLessThan(80);
  // The corners stay anchored to their source colours, so the output is not a
  // blanket recolor or decode artefact.
  expect(p.topLeft.r).toBeGreaterThan(220);
  expect(p.topLeft.g).toBeLessThan(40);
  expect(p.bottomRight.r).toBeGreaterThan(220);
  expect(p.bottomRight.g).toBeGreaterThan(220);
});

test('image-perspective-warp deep-link params drive fields and ffmpeg output', async ({ page }) => {
  await page.goto('/tools/image-perspective-warp/?mode=distort&interpolation=cubic&tl_x=12&tl_y=12&tr_x=56&tr_y=12&bl_x=12&bl_y=56&br_x=56&br_y=56');
  await expect(page.locator('#in-mode')).toHaveValue('distort');
  await expect(page.locator('#in-interpolation')).toHaveValue('cubic');
  await expect(page.locator('#in-tl_x')).toHaveValue('12');
  await expect(page.locator('#in-br_y')).toHaveValue('56');

  await page.setInputFiles('#in-image', QUADRANTS);
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  expect(p.center.r).toBeGreaterThan(220);
  expect(p.center.g).toBeLessThan(80);
  expect(p.center.b).toBeLessThan(80);
});

test('image-perspective-warp preset chip fills the corner fields', async ({ page }) => {
  await page.goto('/tools/image-perspective-warp/');
  await page.getByRole('button', { name: 'Inset 64px frame (distort)' }).click();
  await expect(page.locator('#in-mode')).toHaveValue('distort');
  await expect(page.locator('#in-tl_y')).toHaveValue('12');
  await expect(page.locator('#in-bl_y')).toHaveValue('56');

  await page.setInputFiles('#in-image', QUADRANTS);
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // The tilted preset should move the original center seam leftward/upward, so
  // the exact center is no longer the fixture's original white quadrant.
  expect(p.center.g).toBeLessThan(220);
});

test('image-perspective-warp rejects a collapsed quadrilateral with guidance', async ({ page }) => {
  await page.goto('/tools/image-perspective-warp/');
  await page.fill('#in-tl_x', '50');
  await page.fill('#in-tl_y', '50');
  await page.fill('#in-tr_x', '50');
  await page.fill('#in-tr_y', '50');
  await page.fill('#in-bl_x', '50');
  await page.fill('#in-bl_y', '50');
  await page.fill('#in-br_x', '50');
  await page.fill('#in-br_y', '50');
  await page.setInputFiles('#in-image', WHITE);

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: FF_TIMEOUT });
  await expect(out).toContainText('collinear or collapsed');
});
