import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-bg-replace/ page removes an image's chroma-key
// background (colorkey) and composites the subject onto a transparent, solid,
// or gradient background, in-browser via @ffmpeg/core (needs network for the
// CDN load on the first run).
//
// Fixtures (built with local ffmpeg 6.1):
//   green-screen-64.png / .jpg — 64x64 pure-green (#00ff00) with a 24x24 red
//     (#ff0000) subject block at (20,20). Corners are green (background);
//     center (32,32) is the red subject.
//   blue-screen-64.png — same layout on a pure-blue (#0000ff) background.
// So keying the background out and filling with color X puts X in the corners
// and leaves the red subject in the center — an exact, checkable transform.

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
      topLeft: px(0, 0),
      bottomLeft: px(0, img.naturalHeight - 1),
      topRight: px(img.naturalWidth - 1, 0),
      center: px(Math.floor(img.naturalWidth / 2), Math.floor(img.naturalHeight / 2)),
    };
  }, dataUrl);
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 }); // ffmpeg CDN on first run
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

const fixture = (name: string) => path.resolve(__dirname, 'fixtures', name);

test('default (green screen → solid white) keys the green corners white, keeps the subject', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Dimensions unchanged.
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // The green background became the default white fill…
  expect(p.topLeft.r).toBeGreaterThan(230);
  expect(p.topLeft.g).toBeGreaterThan(230);
  expect(p.topLeft.b).toBeGreaterThan(230);
  expect(p.topLeft.a).toBe(255); // solid → opaque
  // …and the red subject survives in the center.
  expect(p.center.r).toBeGreaterThan(200);
  expect(p.center.g).toBeLessThan(60);
  expect(p.center.b).toBeLessThan(60);
});

test('deep-link ?bg_color=%230000ff fills the keyed background blue (long hex reaches ffmpeg)', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/?bg_color=%230000ff');
  await expect(page.locator('#in-bg_color')).toHaveValue('#0000ff');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Corner is now the deep-linked blue, proving the query value drove ffmpeg.
  expect(p.topLeft.b).toBeGreaterThan(200);
  expect(p.topLeft.r).toBeLessThan(60);
  expect(p.topLeft.g).toBeLessThan(60);
  // Subject stays red (distinct from the blue background).
  expect(p.center.r).toBeGreaterThan(200);
  expect(p.center.b).toBeLessThan(60);
});

test('short-hex key color ?key_color=%230f0 still keys the green (color field never numeric-coerced)', async ({ page }) => {
  // "#0f0" is the 3-digit form of the green screen. It must expand to #00ff00
  // and key the background out (corner → default white fill), not be mangled.
  await page.goto('/tools/image-bg-replace/?key_color=%230f0');
  await expect(page.locator('#in-key_color')).toHaveValue('#0f0');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Green removed → white fill in the corners, red subject preserved.
  expect(p.topLeft.g).toBeGreaterThan(230);
  expect(p.topLeft.r).toBeGreaterThan(230);
  expect(p.center.r).toBeGreaterThan(200);
  expect(p.center.g).toBeLessThan(60);
});

test('transparent background keeps the alpha (corner see-through, subject opaque)', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.selectOption('#in-bg_type', 'transparent');
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/);
  const p = await samplePixels(page, src);
  // Keyed corner is fully transparent; the red subject stays opaque.
  expect(p.topLeft.a).toBeLessThan(30);
  expect(p.center.a).toBe(255);
  expect(p.center.r).toBeGreaterThan(200);
});

test('gradient (vertical, blue → white) fills the keyed background with a top-to-bottom blend', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.selectOption('#in-bg_type', 'gradient');
  await page.fill('#in-bg_color', '#0000ff');
  await page.fill('#in-bg_color2', '#ffffff');
  await page.selectOption('#in-direction', 'vertical');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Top edge near the start color (blue), bottom edge near the end color (white).
  expect(p.topLeft.b).toBeGreaterThan(180);
  expect(p.topLeft.r).toBeLessThan(90);
  expect(p.bottomLeft.r).toBeGreaterThan(180);
  expect(p.bottomLeft.g).toBeGreaterThan(180);
  expect(p.bottomLeft.b).toBeGreaterThan(180);
  // Subject still red in the middle.
  expect(p.center.r).toBeGreaterThan(180);
  expect(p.center.g).toBeLessThan(80);
});

test('gradient direction=horizontal blends left-to-right instead', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.selectOption('#in-bg_type', 'gradient');
  await page.fill('#in-bg_color', '#0000ff');
  await page.fill('#in-bg_color2', '#ffffff');
  await page.selectOption('#in-direction', 'horizontal');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Left edge blue (start), right edge white (end) — the horizontal axis.
  expect(p.topLeft.b).toBeGreaterThan(180);
  expect(p.topLeft.r).toBeLessThan(90);
  expect(p.topRight.r).toBeGreaterThan(180);
  expect(p.topRight.g).toBeGreaterThan(180);
  expect(p.topRight.b).toBeGreaterThan(180);
});

test('JPEG input works end-to-end (secondary input format)', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/?bg_color=%230000ff');
  await page.setInputFiles('#in-image', fixture('green-screen-64.jpg'));

  const p = await samplePixels(page, await outputSrc(page));
  // JPEG is lossy, so widen the windows but still prove blue bg + red subject.
  expect(p.topLeft.b).toBeGreaterThan(150);
  expect(p.topLeft.r).toBeLessThan(90);
  expect(p.center.r).toBeGreaterThan(150);
  expect(p.center.b).toBeLessThan(100);
});

test('blue-screen input with key_color=#0000ff removes the blue', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/?key_color=%230000ff&bg_type=transparent');
  await expect(page.locator('#in-key_color')).toHaveValue('#0000ff');
  await page.setInputFiles('#in-image', fixture('blue-screen-64.png'));

  const src = await outputSrc(page);
  const p = await samplePixels(page, src);
  expect(p.topLeft.a).toBeLessThan(30); // blue keyed → transparent
  expect(p.center.a).toBe(255); // red subject kept
  expect(p.center.r).toBeGreaterThan(200);
});

test('format=jpg outputs JPEG; format=webp outputs WebP; keep preserves PNG', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));
  let src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');

  await page.selectOption('#in-format', 'webp');
  // Re-run picks up the new format.
  src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/webp/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.webp');

  await page.selectOption('#in-format', 'keep');
  src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/png/); // png in → png out
});

test('similarity=100 (cap boundary) still marshals through and renders', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.fill('#in-similarity', '100');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));
  // At the maximum the key is very aggressive, but it must still produce a
  // valid image (proves the boundary value reaches ffmpeg, no crash).
  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\//);
});

test('transparent + jpg output is rejected with guidance', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.selectOption('#in-bg_type', 'transparent');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 90_000 });
  await expect(out).toContainText('png or webp');
});

test('preset chip "Green screen → transparent PNG" fills fields and produces a cut-out', async ({ page }) => {
  await page.goto('/tools/image-bg-replace/');
  await page.getByRole('button', { name: 'Green screen → transparent PNG' }).click();
  await expect(page.locator('#in-bg_type')).toHaveValue('transparent');
  await expect(page.locator('#in-key_color')).toHaveValue('#00ff00');

  await page.setInputFiles('#in-image', fixture('green-screen-64.png'));
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.topLeft.a).toBeLessThan(30);
  expect(p.center.a).toBe(255);
});
