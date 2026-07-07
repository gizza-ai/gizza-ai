import { test, expect } from './fixtures';
import path from 'node:path';

// The /tools/image-resize-to-filesize/ page binary-searches the encoder quality
// (via ffmpeg.wasm from jsDelivr — needs network) to land an image at or under a
// target KB. Its page/custom.js drives the search loop and paints the result.
// Fixtures: photo-512.png (512x512 testsrc, ~9-26 KB as JPEG) and photo-320.jpg.

const PNG = path.resolve(__dirname, 'fixtures/photo-512.png');
const JPG = path.resolve(__dirname, 'fixtures/photo-320.jpg');

// Decode a data URL: natural dimensions + the real encoded byte length.
async function decode(page, src: string) {
  return page.evaluate(async (dataUrl: string) => {
    const img = new Image();
    await new Promise((res, rej) => {
      img.onload = res;
      img.onerror = rej;
      img.src = dataUrl;
    });
    const b64 = dataUrl.split(',')[1] || '';
    return { w: img.naturalWidth, h: img.naturalHeight, bytes: atob(b64).length };
  }, src);
}

test('hits the target KB as JPEG and preserves dimensions', async ({ page }) => {
  await page.goto('/tools/image-resize-to-filesize/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_kb', '14');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-file', PNG);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/jpeg/);

  const r = await decode(page, src!);
  expect(r.w).toBe(512); // no max_width → dimensions unchanged
  expect(r.h).toBe(512);
  expect(r.bytes).toBeLessThanOrEqual(14 * 1024); // actually under the budget
  expect(r.bytes).toBeGreaterThan(2 * 1024); // and a real image, not empty
  await expect(page.locator('#tool-output')).toContainText('Done');
});

test('webp output from a jpeg input, under the target', async ({ page }) => {
  await page.goto('/tools/image-resize-to-filesize/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_kb', '8');
  await page.selectOption('#in-format', 'webp');
  await page.setInputFiles('#in-file', JPG); // secondary input format: jpeg

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/webp/);

  const r = await decode(page, src!);
  expect(r.bytes).toBeLessThanOrEqual(8 * 1024);
  expect(r.w).toBe(320);
});

test('max_width shrinks the image on the page', async ({ page }) => {
  await page.goto('/tools/image-resize-to-filesize/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-target_kb', '30');
  await page.selectOption('#in-format', 'jpg');
  await page.fill('#in-max_width', '128');
  await page.setInputFiles('#in-file', PNG);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  const r = await decode(page, src!);
  expect(r.w).toBe(128); // width capped by the scale filter
  expect(r.h).toBe(128); // square source stays square
});

test('deep-link pre-fills params and runs (?target_kb=&format=webp)', async ({ page }) => {
  await page.goto('/tools/image-resize-to-filesize/?target_kb=14&format=webp');
  await page.waitForSelector('#in-file');
  // Scalar fields are prefilled by the shared driver before custom.js setup.
  await expect(page.locator('#in-target_kb')).toHaveValue('14');
  await expect(page.locator('#in-format')).toHaveValue('webp');
  await page.setInputFiles('#in-file', PNG);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/webp/);
  const r = await decode(page, src!);
  expect(r.bytes).toBeLessThanOrEqual(14 * 1024);
});

test('reports best-effort when the target is unreachable', async ({ page }) => {
  await page.goto('/tools/image-resize-to-filesize/');
  await page.waitForSelector('#in-file');
  // 512x512 testsrc as JPEG is ~9 KB even at the lowest quality — 8 KB can't be
  // met at full resolution, so the tool returns the smallest and says so.
  await page.fill('#in-target_kb', '8');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-file', PNG);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  await expect(page.locator('#tool-output')).toContainText(/Smallest reachable|still over/);
});
