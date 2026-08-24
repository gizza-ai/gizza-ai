import { test, expect } from './fixtures';
import path from 'node:path';

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
  expect(src).toMatch(/^data:image\/png/);
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
  expect(src).toMatch(/^data:image\/jpeg/);
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
