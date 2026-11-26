import { test, expect, readResult } from './fixtures';
import path from 'node:path';
import { outputSize } from './output-size';

const JPEG_FIXTURE = path.resolve(__dirname, 'fixtures/white-64x64.jpg');

async function decodeImage(page, src: string): Promise<{ w: number; h: number; alpha: number }> {
  return page.evaluate(async (dataUrl) => {
    const img = new Image();
    await new Promise<void>((res, rej) => {
      img.onload = () => res();
      img.onerror = () => rej(new Error('image decode failed'));
      img.src = dataUrl;
    });
    const canvas = document.createElement('canvas');
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const alpha = ctx.getImageData(0, 0, 1, 1).data[3];
    return { w: img.naturalWidth, h: img.naturalHeight, alpha };
  }, src);
}

test('cmyk-jpeg-to-rgb converts a JPEG to default RGB PNG output', async ({ page }) => {
  await page.goto('/tools/cmyk-jpeg-to-rgb/');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-format')).toHaveValue('png');
  await expect(page.locator('#in-quality')).toHaveValue('90');
  await expect(page.locator('#in-chroma')).toHaveValue('4:2:0');

  await page.setInputFiles('#in-file', JPEG_FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect((await readResult(page, src)).mime).toMatch(/^image\/png/);
  const meta = await decodeImage(page, src!);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
  expect(meta.alpha).toBe(255);
});

test('cmyk-jpeg-to-rgb deep link accepts JPEG 4:4:4 output settings', async ({ page }) => {
  await page.goto('/tools/cmyk-jpeg-to-rgb/?format=jpeg&quality=95&chroma=4%3A4%3A4');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-format')).toHaveValue('jpeg', { timeout: 15_000 });
  await expect(page.locator('#in-quality')).toHaveValue('95');
  await expect(page.locator('#in-chroma')).toHaveValue('4:4:4');

  await page.setInputFiles('#in-file', JPEG_FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect((await readResult(page, src)).mime).toMatch(/^image\/jpeg/);
  const meta = await decodeImage(page, src!);
  expect(meta.w).toBe(64);
  expect(meta.h).toBe(64);
});

test('cmyk-jpeg-to-rgb page ships runnable CLI, labels, and preset chips', async ({ page }) => {
  await page.goto('/tools/cmyk-jpeg-to-rgb/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool cmyk-jpeg-to-rgb 'url=https://example.com/input' 'format=png' 'quality=90' 'chroma=4:2:0'"
  );
  await expect(page.locator('#in-format option[value="png"]')).toHaveText('PNG — lossless (default)');
  await expect(page.locator('#in-format option[value="webp"]')).toHaveText('WebP — smallest for the web');
  await expect(page.locator('#in-chroma option[value="4:4:4"]')).toHaveText('4:4:4 — full colour detail for type & logos');
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
});

// Pins the quality DIRECTION per lossy encoder against the browser's real
// ffmpeg.wasm build: mjpeg's `-q:v` is inverted (2 best – 31 worst) while
// libwebp's `-quality` is 0 worst – 100 best, and the page's 1-100 quality
// must mean "higher = better" for both. A detailed photo, not the flat white
// fixture, so the encoder's quality actually changes the size.
const PHOTO_FIXTURE = path.resolve(__dirname, 'fixtures/photo-320.jpg');

function convertedBytes(page, format: string, quality: number) {
  return outputSize(
    page,
    `/tools/cmyk-jpeg-to-rgb/?format=${format}&quality=${quality}&chroma=4%3A2%3A0`,
    '#in-file',
    PHOTO_FIXTURE,
    { '#in-format': format, '#in-quality': String(quality) },
  );
}

for (const [format, mime] of [
  ['webp', 'image/webp'],
  ['jpeg', 'image/jpeg'],
] as const) {
  test(`cmyk-jpeg-to-rgb: higher quality gives a larger ${format}`, async ({ page }) => {
    const lo = await convertedBytes(page, format, 10);
    const hi = await convertedBytes(page, format, 90);
    expect(lo.mime).toBe(mime);
    expect(hi.mime).toBe(mime);
    console.log(`cmyk-jpeg-to-rgb ${format}: q10=${lo.bytes} B, q90=${hi.bytes} B`);
    expect(hi.bytes).toBeGreaterThan(lo.bytes);
  });
}
