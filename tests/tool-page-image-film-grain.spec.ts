import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-film-grain/ page overlays analog film grain on an
// uploaded image in-browser via ffmpeg's `noise` filter (single-threaded
// @ffmpeg/core from a CDN — needs network). Monochrome (default) noises only
// the luma plane (neutral gray speckle); colored grain noises every channel.
//
// The fixtures are uniform 64x64 solids, so a no-op would leave EVERY pixel
// identical. Real grain spreads the samples apart — that spread is what these
// tests assert, so a silent pass-through still fails.
const GRAY = 'fixtures/gray-64x64.png'; // solid mid-gray (128,128,128)
const FF_TIMEOUT = 90_000;

// Decode the produced data URL on a canvas and sample a grid of pixels so we
// can measure how far grain has spread the (originally uniform) image.
async function sampleGrid(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = src; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const lumas: number[] = [];
    const chromaDiffs: number[] = []; // |r-g| + |g-b| per sample — 0 when neutral
    // Walk an 8x8 grid across the image.
    for (let gy = 0; gy < 8; gy++) {
      for (let gx = 0; gx < 8; gx++) {
        const x = Math.min(img.naturalWidth - 1, gx * Math.floor(img.naturalWidth / 8));
        const y = Math.min(img.naturalHeight - 1, gy * Math.floor(img.naturalHeight / 8));
        const d = ctx.getImageData(x, y, 1, 1).data;
        lumas.push(0.299 * d[0] + 0.587 * d[1] + 0.114 * d[2]);
        chromaDiffs.push(Math.abs(d[0] - d[1]) + Math.abs(d[1] - d[2]));
      }
    }
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      lumaSpread: Math.max(...lumas) - Math.min(...lumas),
      meanLuma: lumas.reduce((a, b) => a + b, 0) / lumas.length,
      maxChromaDiff: Math.max(...chromaDiffs),
    };
  }, dataUrl);
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: FF_TIMEOUT }); // ffmpeg loads from CDN on first run
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

test('image-film-grain page overlays neutral grain at the default amount', async ({ page }) => {
  await page.goto('/tools/image-film-grain/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, GRAY));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/); // keep (default): PNG in → PNG out
  const s = await sampleGrid(page, src);
  // Grain never crops or resizes.
  expect(s.w).toBe(64);
  expect(s.h).toBe(64);
  // Uniform gray input would have lumaSpread 0; the default amount 20 spreads
  // pixels apart — proof real grain was applied, not a pass-through.
  expect(s.lumaSpread).toBeGreaterThan(5);
  // Monochrome (default) noises luma only, so the samples stay neutral gray and
  // the overall brightness is unchanged.
  expect(s.maxChromaDiff).toBeLessThan(12);
  expect(Math.abs(s.meanLuma - 128)).toBeLessThan(20);
});

test('image-film-grain deep-link ?amount=70 drives much heavier grain than the default', async ({
  page,
}) => {
  // Establish the default spread first, then the deep-linked heavy spread, and
  // assert the query param really reached ffmpeg by comparing them.
  await page.goto('/tools/image-film-grain/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, GRAY));
  const light = await sampleGrid(page, await outputSrc(page));

  await page.goto('/tools/image-film-grain/?amount=70');
  // The query param must land in the field (and mirror to the slider) before the run.
  await expect(page.locator('#in-amount')).toHaveValue('70');
  await page.setInputFiles('#in-image', path.resolve(__dirname, GRAY));
  const heavy = await sampleGrid(page, await outputSrc(page));

  expect(heavy.w).toBe(64);
  expect(heavy.h).toBe(64);
  // Amount 70 spreads the samples far wider than the default 20 — only possible
  // if the deep-linked value drove the noise strength.
  expect(heavy.lumaSpread).toBeGreaterThan(light.lumaSpread + 10);
});

test('image-film-grain colored grain tints the speckle (monochrome unchecked)', async ({ page }) => {
  await page.goto('/tools/image-film-grain/');
  await page.fill('#in-amount', '60');
  await page.uncheck('#in-monochrome'); // default is checked → exercise the colored path
  await page.setInputFiles('#in-image', path.resolve(__dirname, GRAY));

  const s = await sampleGrid(page, await outputSrc(page));
  expect(s.w).toBe(64);
  // Colored grain noises every channel independently, so at least some samples
  // pick up color — the R/G/B channels drift apart (never true for neutral grain).
  expect(s.maxChromaDiff).toBeGreaterThan(20);
});

test('image-film-grain format=jpg converts the output to JPEG', async ({ page }) => {
  await page.goto('/tools/image-film-grain/');
  await page.selectOption('#in-format', 'jpg');
  await page.fill('#in-amount', '40');
  await page.setInputFiles('#in-image', path.resolve(__dirname, GRAY));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/); // format conversion really happened
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
  const s = await sampleGrid(page, src);
  expect(s.w).toBe(64);
  expect(s.lumaSpread).toBeGreaterThan(5); // grain survived the JPEG encode
});
