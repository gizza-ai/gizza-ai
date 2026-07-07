import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-shrink-for-sharing/ page downscales + strips
// metadata + re-encodes an uploaded image in-browser via ffmpeg (single-threaded
// @ffmpeg/core from jsDelivr — needs network). Fixtures white-64x64.png /
// white-64x64.jpg are exactly 64x64 so downscale targets are exact.
const PNG = 'fixtures/white-64x64.png';
const JPG = 'fixtures/white-64x64.jpg';
const FF_TIMEOUT = 90_000;

// Decode the produced data URL and return its real pixel dimensions + mime.
async function decode(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: FF_TIMEOUT });
  const src = await media.getAttribute('src');
  const dims = await page.evaluate(async (dataUrl: string) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = dataUrl; });
    return { w: img.naturalWidth, h: img.naturalHeight };
  }, src as string);
  return { src: src as string, ...dims };
}

test('downscale 64→32, convert PNG→WebP, metadata OFF (non-default checkbox)', async ({ page }) => {
  await page.goto('/tools/image-shrink-for-sharing/');
  await page.waitForSelector('#in-image');
  await page.fill('#in-max_dimension', '32');
  await page.fill('#in-quality', '80');
  await page.selectOption('#in-format', 'webp');
  await page.uncheck('#in-strip_metadata'); // default is checked → exercise the OFF path
  await page.setInputFiles('#in-image', path.resolve(__dirname, PNG));
  const out = await decode(page);
  expect(out.src).toMatch(/^data:image\/webp/); // format conversion really happened
  expect(out.w).toBe(32); // longest side capped to 32, aspect kept
  expect(out.h).toBe(32);
});

test('keep format on a JPEG input (secondary format), downscale 64→48', async ({ page }) => {
  await page.goto('/tools/image-shrink-for-sharing/');
  await page.waitForSelector('#in-image');
  await page.fill('#in-max_dimension', '48');
  await page.selectOption('#in-format', 'keep');
  await page.setInputFiles('#in-image', path.resolve(__dirname, JPG));
  const out = await decode(page);
  expect(out.src).toMatch(/^data:image\/jpeg/); // keep → JPEG stays JPEG
  expect(out.w).toBe(48);
  expect(out.h).toBe(48);
});

test('convert to PNG, cap boundary one-over does NOT upscale (max 65 on 64px)', async ({ page }) => {
  await page.goto('/tools/image-shrink-for-sharing/');
  await page.waitForSelector('#in-image');
  await page.fill('#in-max_dimension', '65'); // one over the 64px source → no upscale
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-image', path.resolve(__dirname, JPG));
  const out = await decode(page);
  expect(out.src).toMatch(/^data:image\/png/); // JPEG→PNG conversion
  expect(out.w).toBe(64); // stays at source size, never enlarged
  expect(out.h).toBe(64);
});

test('convert PNG→JPEG, keep original size (max 0)', async ({ page }) => {
  await page.goto('/tools/image-shrink-for-sharing/');
  await page.waitForSelector('#in-image');
  await page.fill('#in-max_dimension', '0'); // 0 = no resize
  await page.fill('#in-quality', '70');
  await page.selectOption('#in-format', 'jpeg');
  await page.setInputFiles('#in-image', path.resolve(__dirname, PNG));
  const out = await decode(page);
  expect(out.src).toMatch(/^data:image\/jpeg/);
  expect(out.w).toBe(64); // unchanged dimensions
  expect(out.h).toBe(64);
});

test('deep-link ?param= pre-fills the fields, then runs', async ({ page }) => {
  await page.goto('/tools/image-shrink-for-sharing/?max_dimension=32&quality=80&format=webp');
  await page.waitForSelector('#in-image');
  await expect(page.locator('#in-max_dimension')).toHaveValue('32');
  await expect(page.locator('#in-format')).toHaveValue('webp');
  await page.setInputFiles('#in-image', path.resolve(__dirname, PNG));
  const out = await decode(page);
  expect(out.src).toMatch(/^data:image\/webp/);
  expect(out.w).toBe(32);
  expect(out.h).toBe(32);
});
