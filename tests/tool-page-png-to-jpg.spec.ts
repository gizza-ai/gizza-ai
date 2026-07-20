import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/png-to-jpg/ page flattens an image's transparency onto
// a chosen background color and encodes JPEG, in-browser via @ffmpeg/core
// (needs network for the CDN load on the first run).
//
// Fixtures (built with local ffmpeg):
//   alpha-grad-64.png — 64x64, three vertical bands: x<21 fully transparent
//     red (alpha 0), 21<=x<43 half-transparent red (alpha 128), x>=43 opaque
//     red (#ff0000). Flattening onto color C makes the left band exactly C,
//     the middle band the 50/50 blend of red over C, and keeps the right band
//     red — an exact, checkable transform.
//   photo-512.png — an opaque photograph (for the quality-size check).
//   green-screen-64.jpg — 64x64 opaque JPEG (secondary input format).
// Sample points sit ≥10px from band edges so yuv420 chroma subsampling and
// JPEG loss never cross a boundary: left (8,32), middle (32,32), right (56,32).

async function samplePixels(page: import('@playwright/test').Page, dataUrl: string) {
  return page.evaluate(async (src) => {
    const img = new Image();
    await new Promise((res, rej) => { img.onload = res; img.onerror = rej; img.src = src; });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const px = (x: number, y: number) => {
      const d = ctx.getImageData(x, y, 1, 1).data;
      return { r: d[0], g: d[1], b: d[2], a: d[3] };
    };
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      left: px(8, 32),
      middle: px(32, 32),
      right: px(56, 32),
    };
  }, dataUrl);
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 }); // ffmpeg CDN on first run
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/jpeg/);
  return src!;
}

const fixture = (name: string) => path.resolve(__dirname, 'fixtures', name);

test('default flatten (white): transparent band → white, half-alpha band → 50/50 blend, opaque red kept', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));

  const src = await outputSrc(page);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
  const p = await samplePixels(page, src);
  // Dimensions unchanged.
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Fully transparent band became the default white background…
  expect(p.left.r).toBeGreaterThan(230);
  expect(p.left.g).toBeGreaterThan(230);
  expect(p.left.b).toBeGreaterThan(230);
  expect(p.left.a).toBe(255); // JPEG is opaque
  // …the half-transparent band is red blended onto white (≈ #ff8080)…
  expect(p.middle.r).toBeGreaterThan(220);
  expect(p.middle.g).toBeGreaterThan(90);
  expect(p.middle.g).toBeLessThan(180);
  expect(p.middle.b).toBeGreaterThan(90);
  expect(p.middle.b).toBeLessThan(180);
  // …and the opaque red band survives untouched.
  expect(p.right.r).toBeGreaterThan(200);
  expect(p.right.g).toBeLessThan(60);
  expect(p.right.b).toBeLessThan(60);
});

test('deep-link ?background=%23000000&quality=90 prefills and flattens onto black (long hex)', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/?background=%23000000&quality=90');
  await expect(page.locator('#in-background')).toHaveValue('#000000');
  await expect(page.locator('#in-quality')).toHaveValue('90');
  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Transparent band → black background.
  expect(p.left.r).toBeLessThan(25);
  expect(p.left.g).toBeLessThan(25);
  expect(p.left.b).toBeLessThan(25);
  // Half-alpha red over black ≈ #800000.
  expect(p.middle.r).toBeGreaterThan(90);
  expect(p.middle.r).toBeLessThan(180);
  expect(p.middle.g).toBeLessThan(60);
  expect(p.middle.b).toBeLessThan(60);
  // Opaque red kept.
  expect(p.right.r).toBeGreaterThan(200);
});

test('short-hex background #00f flattens onto blue (color field never numeric-coerced)', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.fill('#in-background', '#00f');
  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Transparent band → pure blue.
  expect(p.left.b).toBeGreaterThan(200);
  expect(p.left.r).toBeLessThan(60);
  expect(p.left.g).toBeLessThan(60);
  // Half-alpha red over blue ≈ purple (#800080).
  expect(p.middle.r).toBeGreaterThan(90);
  expect(p.middle.r).toBeLessThan(180);
  expect(p.middle.b).toBeGreaterThan(90);
  expect(p.middle.b).toBeLessThan(180);
  expect(p.middle.g).toBeLessThan(60);
});

test('named color background "navy" reaches ffmpeg', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.fill('#in-background', 'navy');
  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // navy = #000080: half-intensity blue, no red/green.
  expect(p.left.b).toBeGreaterThan(100);
  expect(p.left.b).toBeLessThan(165);
  expect(p.left.r).toBeLessThan(40);
  expect(p.left.g).toBeLessThan(40);
});

test('quality actually changes the encode: q=100 produces a larger file than q=1', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.fill('#in-quality', '1');
  await page.setInputFiles('#in-image', fixture('photo-512.png'));
  const low = await outputSrc(page);

  // Changing the field re-runs on the already-selected file (cap boundary 100).
  await page.fill('#in-quality', '100');
  await page.locator('#in-quality').blur();
  await expect
    .poll(async () => (await page.locator('#tool-output-media').getAttribute('src'))!.length, {
      timeout: 90_000,
    })
    .toBeGreaterThan(low.length * 2);
});

test('quality=101 (one over the cap) is rejected with a clear error', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.fill('#in-quality', '101');
  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 90_000 });
  await expect(out).toContainText('between 1 and 100');
});

test('JPEG input works end-to-end (secondary input format, opaque passthrough)', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.setInputFiles('#in-image', fixture('green-screen-64.jpg'));

  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Opaque input: the background color never shows — the green corner area
  // (x=8,y=32 is inside the green screen region) stays green, not white.
  expect(p.left.g).toBeGreaterThan(180);
  expect(p.left.r).toBeLessThan(90);
});

test('preset chip "Black background" prefills the fields and drives the flatten', async ({ page }) => {
  await page.goto('/tools/png-to-jpg/');
  await page.getByRole('button', { name: 'Black background' }).click();
  await expect(page.locator('#in-background')).toHaveValue('#000000');
  await expect(page.locator('#in-quality')).toHaveValue('85');

  await page.setInputFiles('#in-image', fixture('alpha-grad-64.png'));
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.left.r).toBeLessThan(25);
  expect(p.left.g).toBeLessThan(25);
  expect(p.left.b).toBeLessThan(25);
  expect(p.right.r).toBeGreaterThan(200);
});
