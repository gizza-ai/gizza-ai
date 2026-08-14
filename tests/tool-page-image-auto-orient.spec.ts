import { test, expect } from './fixtures';
import path from 'node:path';

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 }); // ffmpeg CDN on first run
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

async function imageSize(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => {
      img.onload = res;
      img.onerror = rej;
      img.src = src;
    });
    return { w: img.naturalWidth, h: img.naturalHeight };
  }, dataUrl);
}

const fixture = (name: string) => path.resolve(__dirname, 'fixtures', name);

test('image-auto-orient page forces EXIF orientation 6 and swaps dimensions', async ({ page }) => {
  await page.goto('/tools/image-auto-orient/');
  await page.selectOption('#in-orientation', '6');
  await page.selectOption('#in-format', 'png');
  await page.fill('#in-quality', '90');
  await page.setInputFiles('#in-image', fixture('wide-3x2.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.png');
  expect(await imageSize(page, src)).toEqual({ w: 2, h: 3 });
});

test('image-auto-orient deep-link prefills orientation, format, and quality', async ({ page }) => {
  await page.goto('/tools/image-auto-orient/?orientation=8&format=webp&quality=100');
  await expect(page.locator('#in-orientation')).toHaveValue('8');
  await expect(page.locator('#in-format')).toHaveValue('webp');
  await expect(page.locator('#in-quality')).toHaveValue('100');
  await page.setInputFiles('#in-image', fixture('wide-3x2.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/webp/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.webp');
  expect(await imageSize(page, src)).toEqual({ w: 2, h: 3 });
});

test('image-auto-orient preset chip fills a non-default forced correction', async ({ page }) => {
  await page.goto('/tools/image-auto-orient/');
  await page.getByRole('button', { name: 'Force 90° clockwise' }).click();
  await expect(page.locator('#in-orientation')).toHaveValue('6');
  await expect(page.locator('#in-format')).toHaveValue('same');
  await expect(page.locator('#in-quality')).toHaveValue('90');
});
