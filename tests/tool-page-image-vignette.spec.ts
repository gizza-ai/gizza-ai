import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-vignette/ page darkens (or lightens) the edges of
// an uploaded image in-browser via ffmpeg's vignette filter (single-threaded
// @ffmpeg/core from a CDN — needs network).
//
// Bounds are pre-measured against local ffmpeg 6.1 on the same fixtures:
//   white 64x64, strength 40 (default): corners ~109–116, center 255
//   white 64x64, strength 80:           corners ~2–4,     center 255
//   gray  64x64, strength 80 lighten:   corners 255,      center 128
// Margins are wide so a slightly different browser ffmpeg build still passes,
// while a silent no-op (corner staying at the input's value) still fails.

// Decode the output data URL on a canvas and sample corner + center pixels.
async function samplePixels(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = src; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const px = (x: number, y: number) => ctx.getImageData(x, y, 1, 1).data[0];
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      topLeft: px(0, 0),
      bottomRight: px(img.naturalWidth - 1, img.naturalHeight - 1),
      center: px(Math.floor(img.naturalWidth / 2), Math.floor(img.naturalHeight / 2)),
    };
  }, dataUrl);
}

test('image-vignette page darkens corners at the default strength', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  // ffmpeg loads from CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);

  const p = await samplePixels(page, src!);
  // Dimensions unchanged — a vignette never crops or resizes.
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Input is uniform white (255 everywhere). At the default strength 40 the
  // center stays bright while the corners drop to ~110 (measured 109–116).
  expect(p.center).toBeGreaterThan(230);
  expect(p.topLeft).toBeLessThan(180); // darker than the input's 255 corners
  expect(p.bottomRight).toBeLessThan(180);
  expect(p.topLeft).toBeLessThan(p.center - 60); // measurably darker than center
  expect(p.topLeft).toBeGreaterThan(40); // ...but NOT black: 40 is a soft vignette
});

test('image-vignette deep-link ?strength=80 drives a much darker vignette', async ({ page }) => {
  await page.goto('/tools/image-vignette/?strength=80');
  // The query param must land in the field before the run.
  await expect(page.locator('#in-strength')).toHaveValue('80');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);

  const p = await samplePixels(page, src!);
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Strength 80 corners are nearly black (measured 2–4) — far below the ~110
  // the default produces, so this proves the deep-linked value reached ffmpeg.
  expect(p.center).toBeGreaterThan(230);
  expect(p.topLeft).toBeLessThan(60);
  expect(p.bottomRight).toBeLessThan(60);
});

test('image-vignette lighten mode brightens the edges instead', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.selectOption('#in-mode', 'lighten');
  await page.fill('#in-strength', '80');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/gray-64x64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);

  const p = await samplePixels(page, src!);
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Input is uniform mid-gray (128). Backward mode brightens with distance:
  // corners saturate to white (measured 255) while the center stays gray.
  expect(p.topLeft).toBeGreaterThan(230);
  expect(p.bottomRight).toBeGreaterThan(230);
  expect(p.center).toBeLessThan(160);
  expect(p.center).toBeGreaterThan(90);
});
