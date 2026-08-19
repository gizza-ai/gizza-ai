import { test, expect } from './fixtures';
import path from 'node:path';

async function sampleImage(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((resolve, reject) => {
      img.onload = resolve;
      img.onerror = reject;
      img.src = src;
    });
    const canvas = document.createElement('canvas');
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
    ctx.drawImage(img, 0, 0);
    const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let bright = 0;
    let dark = 0;
    let total = 0;
    let colorish = 0;
    for (let i = 0; i < data.length; i += 4) {
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      const luma = (r + g + b) / 3;
      if (luma > 180) bright += 1;
      if (luma < 40) dark += 1;
      if (Math.max(r, g, b) - Math.min(r, g, b) > 20) colorish += 1;
      total += 1;
    }
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      brightRatio: bright / total,
      darkRatio: dark / total,
      colorRatio: colorish / total,
    };
  }, dataUrl);
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

const white = path.resolve(__dirname, 'fixtures/white-64x64.png');
const quadrants = path.resolve(__dirname, 'fixtures/quadrants-64x64.png');
const jpg = path.resolve(__dirname, 'fixtures/white-64x64.jpg');

test('edge-detection page produces a real Canny edge-map image', async ({ page }) => {
  await page.goto('/tools/edge-detection/');
  await page.setInputFiles('#in-image', quadrants);
  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/);
  const stats = await sampleImage(page, src);
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
  expect(stats.brightRatio).toBeGreaterThan(0.003);
  expect(stats.brightRatio).toBeLessThan(0.25);
  expect(stats.darkRatio).toBeGreaterThan(0.7);
});

test('edge-detection deep-link wires Sobel + inverted output', async ({ page }) => {
  await page.goto('/tools/edge-detection/?method=sobel&invert=true&format=png');
  await expect(page.locator('#in-method')).toHaveValue('sobel');
  await expect(page.locator('#in-invert')).toBeChecked();
  await page.setInputFiles('#in-image', quadrants);
  const stats = await sampleImage(page, await outputSrc(page));
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
  expect(stats.brightRatio).toBeGreaterThan(0.7);
});

test('edge-detection advertised method and format controls stay wired', async ({ page }) => {
  await page.goto('/tools/edge-detection/');
  await page.selectOption('#in-method', 'colormix');
  await page.selectOption('#in-format', 'webp');
  await page.fill('#in-low', '0');
  await page.fill('#in-high', '1');
  await page.fill('#in-blur', '10');
  await expect(page.locator('#in-low-slider')).toHaveValue('0');
  await expect(page.locator('#in-high-slider')).toHaveValue('1');
  await expect(page.locator('#in-blur-slider')).toHaveValue('10');
  await page.setInputFiles('#in-image', quadrants);
  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/webp/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.webp');
  const stats = await sampleImage(page, src);
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
  expect(stats.colorRatio).toBeGreaterThan(0.05);
});

test('edge-detection accepts JPEG input and JPEG output', async ({ page }) => {
  await page.goto('/tools/edge-detection/?format=jpg&method=sobel');
  await expect(page.locator('#in-format')).toHaveValue('jpg');
  await page.setInputFiles('#in-image', jpg);
  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
  const stats = await sampleImage(page, src);
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
});

test('edge-detection preset chips and generated CLI example are generic', async ({ page }) => {
  await page.goto('/tools/edge-detection/');
  await page.getByRole('button', { name: 'Coloring page' }).click();
  await expect(page.locator('#in-method')).toHaveValue('canny');
  await expect(page.locator('#in-low')).toHaveValue('0.15');
  await expect(page.locator('#in-high')).toHaveValue('0.4');
  await expect(page.locator('#in-blur')).toHaveValue('1.5');
  await expect(page.locator('#in-invert')).toBeChecked();

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool edge-detection');
  expect(cli).toContain('url=https://example.com/input');
  expect(cli).toContain('method=canny');
  expect(cli).toContain('format=png');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');

  await page.setInputFiles('#in-image', white);
  const stats = await sampleImage(page, await outputSrc(page));
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
  expect(stats.brightRatio).toBeGreaterThan(0.7);
});
