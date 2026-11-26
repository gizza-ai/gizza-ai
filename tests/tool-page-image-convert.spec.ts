import { test, expect } from './fixtures';
import path from 'node:path';
import { outputSize } from './output-size';
import { ALPHA_SQUARE, alphaReport, expectFlattenedOnWhite, expectRed, resultUrl } from './alpha-square';

// The generated /tools/image-convert/ page transcodes an uploaded image in the
// browser via ffmpeg.wasm (single-threaded @ffmpeg/core from jsDelivr — needs
// network). These specs pin the QUALITY DIRECTION per lossy encoder against
// the real browser ffmpeg build, not a local ffmpeg: mjpeg's `-q:v` is an
// inverted 2 (best) – 31 (worst) scale, libwebp's `-quality` is 0 (worst) –
// 100 (best), and the page's 1-100 quality must mean "higher = better" for both.

const PHOTO = path.resolve(__dirname, 'fixtures/photo-512.png');

function convertedBytes(page, format: string, quality: number) {
  return outputSize(page, `/tools/image-convert/?format=${format}&quality=${quality}`, '#in-image', PHOTO, {
    '#in-format': format,
    '#in-quality': String(quality),
  });
}

for (const [format, mime] of [
  ['webp', 'image/webp'],
  ['jpeg', 'image/jpeg'],
] as const) {
  test(`image-convert: higher quality gives a larger ${format}`, async ({ page }) => {
    const lo = await convertedBytes(page, format, 10);
    const hi = await convertedBytes(page, format, 90);
    expect(lo.mime).toBe(mime);
    expect(hi.mime).toBe(mime);
    console.log(`image-convert ${format}: q10=${lo.bytes} B, q90=${hi.bytes} B`);
    expect(hi.bytes).toBeGreaterThan(lo.bytes);
  });
}

test('image-convert converts to lossless PNG', async ({ page }) => {
  const out = await convertedBytes(page, 'png', 85);
  expect(out.mime).toBe('image/png');
  expect(out.bytes).toBeGreaterThan(0);
});

// JPEG has no alpha: a transparent PNG (e.g. a background removal's cutout) is
// flattened onto white instead of exposing the color stored under its
// transparent pixels — the green here, the removed background in real use.
test('image-convert flattens a transparent PNG onto white for JPEG', async ({ page }) => {
  await page.goto('/tools/image-convert/?format=jpeg&quality=90');
  await expect(page.locator('#in-format')).toHaveValue('jpeg', { timeout: 15_000 });
  await page.setInputFiles('#in-image', ALPHA_SQUARE);
  const r = await alphaReport(page, await resultUrl(page));
  expectFlattenedOnWhite(r);
  expectRed(r.center);
});

test('image-convert keeps transparency for PNG and WebP', async ({ page }) => {
  for (const format of ['png', 'webp']) {
    await page.goto(`/tools/image-convert/?format=${format}`);
    await expect(page.locator('#in-format')).toHaveValue(format, { timeout: 15_000 });
    await page.setInputFiles('#in-image', ALPHA_SQUARE);
    const r = await alphaReport(page, await resultUrl(page));
    expect(r.corner[3], `${format} background stays transparent`).toBe(0);
    expect(r.mismatches, `${format} alpha mask unchanged`).toBe(0);
    expectRed(r.center);
  }
});
