import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-convert/ page transcodes an uploaded image in the
// browser via ffmpeg.wasm (single-threaded @ffmpeg/core from jsDelivr — needs
// network). These specs pin the QUALITY DIRECTION per lossy encoder against
// the real browser ffmpeg build, not a local ffmpeg: mjpeg's `-q:v` is an
// inverted 2 (best) – 31 (worst) scale, libwebp's `-quality` is 0 (worst) –
// 100 (best), and the page's 1-100 quality must mean "higher = better" for both.

const PHOTO = path.resolve(__dirname, 'fixtures/photo-512.png');

async function convertedBytes(page, format: string, quality: number): Promise<{ mime: string; bytes: number }> {
  await page.goto(`/tools/image-convert/?format=${format}&quality=${quality}`);
  await page.waitForSelector('#in-image');
  await expect(page.locator('#in-format')).toHaveValue(format, { timeout: 15_000 });
  await expect(page.locator('#in-quality')).toHaveValue(String(quality));
  await page.setInputFiles('#in-image', PHOTO);

  // ffmpeg loads from CDN on first run; allow generous time.
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = (await media.getAttribute('src'))!;
  return page.evaluate(async (url) => {
    const blob = await (await fetch(url)).blob();
    return { mime: blob.type, bytes: blob.size };
  }, src);
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
