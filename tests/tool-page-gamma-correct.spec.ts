import { test, expect } from './fixtures';
import path from 'node:path';

const GRAY = path.resolve(__dirname, 'fixtures/gray-64x64.png');
const WHITE = path.resolve(__dirname, 'fixtures/white-64x64.png');
const FF_TIMEOUT = 90_000;

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: FF_TIMEOUT });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

async function sampleCenter(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = src; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(Math.floor(img.naturalWidth / 2), Math.floor(img.naturalHeight / 2), 1, 1).data;
    return { w: img.naturalWidth, h: img.naturalHeight, r: d[0], g: d[1], b: d[2] };
  }, dataUrl);
}

test('gamma-correct page brightens mid-gray at gamma 1.8', async ({ page }) => {
  await page.goto('/tools/gamma-correct/');
  await page.fill('#in-gamma', '1.8');
  await page.setInputFiles('#in-file', GRAY);

  const p = await sampleCenter(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Input gray fixture is 128. Gamma 1.8 should lift midtones substantially.
  expect(p.r).toBeGreaterThan(165);
  expect(p.g).toBeGreaterThan(165);
  expect(p.b).toBeGreaterThan(165);
});

test('gamma-correct deep-link gamma=0.5 darkens mid-gray and mirrors slider', async ({ page }) => {
  await page.goto('/tools/gamma-correct/?gamma=0.5&format=png');
  await expect(page.locator('#in-gamma')).toHaveValue('0.5');
  await expect(page.locator('#in-gamma-slider')).toHaveValue('0.5');
  await expect(page.locator('#in-format')).toHaveValue('png');
  await page.setInputFiles('#in-file', GRAY);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.png');
  const p = await sampleCenter(page, src);
  expect(p.r).toBeLessThan(90);
  expect(p.g).toBeLessThan(90);
  expect(p.b).toBeLessThan(90);
});

test('gamma-correct per-channel gamma warms the image', async ({ page }) => {
  await page.goto('/tools/gamma-correct/');
  await page.fill('#in-gamma_r', '1.8');
  await page.fill('#in-gamma_b', '0.5');
  await page.setInputFiles('#in-file', GRAY);

  const p = await sampleCenter(page, await outputSrc(page));
  expect(p.r).toBeGreaterThan(p.g + 35);
  expect(p.g).toBeGreaterThan(p.b + 35);
});

test('gamma-correct preset chip and jpg format run end-to-end', async ({ page }) => {
  await page.goto('/tools/gamma-correct/');
  await page.getByRole('button', { name: 'Protect highlights' }).click();
  await expect(page.locator('#in-gamma')).toHaveValue('2.2');
  await expect(page.locator('#in-gamma_weight')).toHaveValue('0.35');
  await expect(page.locator('#in-format')).toHaveValue('jpg');
  await page.setInputFiles('#in-file', WHITE);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
});

test('gamma-correct rejects out-of-range gamma with guidance', async ({ page }) => {
  await page.goto('/tools/gamma-correct/');
  await page.fill('#in-gamma', '0');
  await page.setInputFiles('#in-file', GRAY);

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: FF_TIMEOUT });
  await expect(out).toContainText('between 0.1 and 10');
});
