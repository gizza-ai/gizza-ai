import { test, expect } from './fixtures';
import path from 'node:path';

async function dataUrlBytes(src: string): Promise<Buffer> {
  const match = src.match(/^data:([^;,]+)?(;base64)?,(.*)$/);
  expect(match, `expected data URL, got ${src.slice(0, 80)}`).toBeTruthy();
  const [, , isBase64, body] = match!;
  return isBase64 ? Buffer.from(body, 'base64') : Buffer.from(decodeURIComponent(body), 'binary');
}

test('bmp-converter page writes a real 8-bit BMP and honors query params', async ({ page }) => {
  await page.goto('/tools/bmp-converter/?format=bmp&bit_depth=8&colors=2&dither=none&grayscale=true&background=%23f00');
  await page.waitForSelector('#in-image');

  await expect(page.locator('#in-format')).toHaveValue('bmp');
  await expect(page.locator('#in-bit_depth')).toHaveValue('8');
  await expect(page.locator('#in-colors')).toHaveValue('2');
  await expect(page.locator('#in-dither')).toHaveValue('none');
  await expect(page.locator('#in-grayscale')).toBeChecked();
  await expect(page.locator('#in-background')).toHaveValue('#f00');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/alpha-grad-64.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/bmp/);
  const bytes = await dataUrlBytes(src!);
  expect(bytes.subarray(0, 2).toString('ascii')).toBe('BM');
  expect(bytes.readUInt16LE(28)).toBe(8);
});

test('bmp-converter page writes a real PNG for reverse conversion mode', async ({ page }) => {
  await page.goto('/tools/bmp-converter/?format=png');
  await page.waitForSelector('#in-image');
  await expect(page.locator('#in-format')).toHaveValue('png');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/red-2x2.png'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const bytes = await dataUrlBytes(src!);
  expect(bytes.subarray(0, 8)).toEqual(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]));
});
