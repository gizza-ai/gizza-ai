import { test, expect } from './fixtures';
import path from 'node:path';

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

async function imageStats(page: import('@playwright/test').Page, dataUrl: string) {
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
    const center = ctx.getImageData(Math.floor(canvas.width / 2), Math.floor(canvas.height / 2), 1, 1).data;
    const all = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
    let dark = 0;
    let total = 0;
    for (let i = 0; i < all.length; i += 4) {
      const luma = (all[i] + all[i + 1] + all[i + 2]) / 3;
      if (luma < 40) dark += 1;
      total += 1;
    }
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      center: [center[0], center[1], center[2]],
      darkRatio: dark / total,
    };
  }, dataUrl);
}

const speck = path.resolve(__dirname, 'fixtures/salt-pepper-9x9.png');
const jpeg = path.resolve(__dirname, 'fixtures/white-64x64.jpg');

test('image-median-denoise removes a black impulse pixel and returns a real PNG', async ({ page }) => {
  await page.goto('/tools/image-median-denoise/');
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-image', speck);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/);
  const stats = await imageStats(page, src);
  expect(stats.w).toBe(9);
  expect(stats.h).toBe(9);
  expect(stats.center[0]).toBeGreaterThan(240);
  expect(stats.center[1]).toBeGreaterThan(240);
  expect(stats.center[2]).toBeGreaterThan(240);
  expect(stats.darkRatio).toBe(0);
});

test('image-median-denoise deep-link wires target, channels, passes, quality, and metadata stripping', async ({ page }) => {
  await page.goto('/tools/image-median-denoise/?radius=2&target=dark&channels=luma&passes=2&format=png&quality=88&strip_metadata=true');
  await expect(page.locator('#in-radius')).toHaveValue('2');
  await expect(page.locator('#in-target')).toHaveValue('dark');
  await expect(page.locator('#in-channels')).toHaveValue('luma');
  await expect(page.locator('#in-passes')).toHaveValue('2');
  await expect(page.locator('#in-format')).toHaveValue('png');
  await expect(page.locator('#in-quality')).toHaveValue('88');
  await expect(page.locator('#in-strip_metadata')).toBeChecked();

  await page.setInputFiles('#in-image', speck);
  const stats = await imageStats(page, await outputSrc(page));
  expect(stats.w).toBe(9);
  expect(stats.h).toBe(9);
});

test('image-median-denoise advertised format and secondary JPEG input are wired', async ({ page }) => {
  await page.goto('/tools/image-median-denoise/');
  await page.selectOption('#in-target', 'bright');
  await page.selectOption('#in-channels', 'chroma');
  await page.selectOption('#in-format', 'jpg');
  await page.fill('#in-quality', '80');
  await page.check('#in-strip_metadata');
  await page.setInputFiles('#in-image', jpeg);

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
  const stats = await imageStats(page, src);
  expect(stats.w).toBe(64);
  expect(stats.h).toBe(64);
});

test('image-median-denoise preset chips and generated CLI example are generic', async ({ page }) => {
  await page.goto('/tools/image-median-denoise/');
  await page.getByRole('button', { name: 'Scanner dust (5×5)' }).click();
  await expect(page.locator('#in-radius')).toHaveValue('2');
  await expect(page.locator('#in-format')).toHaveValue('png');

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool image-median-denoise');
  expect(cli).toContain('url=https://example.com/input');
  expect(cli).toContain('radius=1');
  expect(cli).toContain('format=keep');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
