import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/youtube-thumbnail-generator/ page grabs ONE frame from an
// uploaded video (@ffmpeg/core from a CDN — needs network), scales+center-crops it
// to a target canvas, draws a colored accent bar on one edge, and overlays an
// outlined headline drawn from a bundled font. Output is a PNG rendered as an
// image, so the media src is a data:image/png URL.
//
// Fixture redblue-64.mp4 is a 64×64 clip whose FIRST frame is solid red (~254,0,0).
// At timestamp 0 the source frame is pure red, and scale+crop to any 16:9-ish
// canvas keeps it solid red — so every non-accent, non-text pixel is a known red
// and each accent bar is a known color on its edge. That makes the output
// deterministic enough to assert dimensions AND pixels, not just "an image showed".

const REDBLUE = path.resolve(__dirname, 'fixtures/redblue-64.mp4');
const WEBM = path.resolve(__dirname, 'fixtures/clip-1s.webm');

// Decode a data:image/ URL and read its natural size + the RGB at one pixel.
async function decodeAt(
  page: import('@playwright/test').Page,
  src: string,
  x: number,
  y: number,
) {
  return page.evaluate(
    async ({ dataUrl, px, py }) => {
      const img = new Image();
      await new Promise((res, rej) => {
        img.onload = res;
        img.onerror = rej;
        img.src = dataUrl;
      });
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(px, py, 1, 1).data;
      return { w: img.naturalWidth, h: img.naturalHeight, r: d[0], g: d[1], b: d[2] };
    },
    { dataUrl: src, px: x, py: y },
  );
}

const isRed = (p: { r: number; g: number; b: number }) =>
  p.r > 200 && p.g < 60 && p.b < 60;
const isGreen = (p: { r: number; g: number; b: number }) =>
  p.g > 200 && p.r < 60 && p.b < 60;

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  return src!;
}

// 1) Core correctness: exact canvas dimensions, a solid (non-blank) red frame in
//    the body, a SHORT-hex (#0f0) green accent on the BOTTOM edge, and text at the
//    TOP so it never contaminates the sampled body/accent pixels.
test('thumbnail page renders exact dimensions, a non-blank frame and a bottom accent', async ({ page }) => {
  await page.goto('/tools/youtube-thumbnail-generator/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-headline', 'BIG NEWS');
  await page.fill('#in-timestamp', '0');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '180');
  await page.selectOption('#in-text_position', 'top');
  await page.selectOption('#in-accent_position', 'bottom');
  await page.fill('#in-accent_color', '#0f0'); // short hex → 0x00FF00
  await page.fill('#in-accent_size', '40');
  await page.setInputFiles('#in-file', REDBLUE);

  const src = await outputSrc(page);

  const body = await decodeAt(page, src, 10, 130); // lower-left body pixel, above accent and away from top text
  expect(body.w).toBe(320);
  expect(body.h).toBe(180);
  expect(isRed(body)).toBe(true); // source frame survived — not blank/black

  const accent = await decodeAt(page, src, 160, 178); // bottom edge
  expect(isGreen(accent)).toBe(true);
});

// 2) Deep-link: ?query pre-fills every field and the upload auto-runs. Also
//    exercises text_position=center, accent_position=left, and a LONG-hex accent.
test('thumbnail page pre-fills from ?query and honors center text + left accent', async ({ page }) => {
  await page.goto(
    '/tools/youtube-thumbnail-generator/?headline=HELLO&timestamp=0&width=480&height=270' +
      '&text_position=center&accent_position=left&accent_color=%2300ff00&accent_size=30',
  );
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-headline')).toHaveValue('HELLO');
  await expect(page.locator('#in-width')).toHaveValue('480');
  await expect(page.locator('#in-height')).toHaveValue('270');
  await expect(page.locator('#in-text_position')).toHaveValue('center');
  await expect(page.locator('#in-accent_position')).toHaveValue('left');
  await expect(page.locator('#in-accent_color')).toHaveValue('#00ff00');
  await page.setInputFiles('#in-file', REDBLUE);

  const src = await outputSrc(page);

  const dims = await decodeAt(page, src, 5, 135); // left edge, mid-height
  expect(dims.w).toBe(480);
  expect(dims.h).toBe(270);
  expect(isGreen(dims)).toBe(true); // left accent bar

  const body = await decodeAt(page, src, 470, 10); // top-right corner, away from the centered text
  expect(isRed(body)).toBe(true);
});

// 3) Accent-position enum matrix (top / right / none) on the red frame: the bar
//    lands on the correct edge, and `none` leaves every edge red.
test('thumbnail page places the accent on top, right, or nowhere per accent_position', async ({ page }) => {
  const run = async (pos: string) => {
    await page.goto('/tools/youtube-thumbnail-generator/');
    await page.waitForSelector('#in-file');
    await page.fill('#in-headline', 'X'); // tiny headline can't reach the sampled edges
    await page.fill('#in-timestamp', '0');
    await page.fill('#in-width', '320');
    await page.fill('#in-height', '180');
    await page.selectOption('#in-accent_position', pos);
    await page.fill('#in-accent_color', '#00ff00');
    await page.fill('#in-accent_size', '40');
    await page.setInputFiles('#in-file', REDBLUE);
    return outputSrc(page);
  };

  const topSrc = await run('top');
  expect(isGreen(await decodeAt(page, topSrc, 5, 2))).toBe(true); // top edge green
  expect(isRed(await decodeAt(page, topSrc, 5, 178))).toBe(true); // bottom edge red

  const rightSrc = await run('right');
  expect(isGreen(await decodeAt(page, rightSrc, 318, 90))).toBe(true); // right edge green
  expect(isRed(await decodeAt(page, rightSrc, 2, 90))).toBe(true); // left edge red

  const noneSrc = await run('none');
  for (const [x, y] of [[2, 2], [318, 2], [2, 178], [318, 178]]) {
    expect(isGreen(await decodeAt(page, noneSrc, x, y))).toBe(false); // no bar anywhere
  }
});

// 4) accent_size cap boundary. With a 320×180 canvas the core caps the bar at
//    max(w,h)/2 = 160: exactly 160 renders, 161 is rejected with a clear error on
//    the page (and no image is produced).
test('thumbnail page accepts the accent_size cap and rejects one over it', async ({ page }) => {
  // At the cap (160) → renders a valid 320×180 PNG.
  await page.goto('/tools/youtube-thumbnail-generator/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-headline', 'CAP');
  await page.fill('#in-timestamp', '0');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '180');
  await page.fill('#in-accent_size', '160');
  await page.setInputFiles('#in-file', REDBLUE);
  const src = await outputSrc(page);
  const dims = await decodeAt(page, src, 0, 0);
  expect(dims.w).toBe(320);
  expect(dims.h).toBe(180);

  // One over the cap (161) → error surfaced on the page, no media.
  await page.goto('/tools/youtube-thumbnail-generator/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-headline', 'CAP');
  await page.fill('#in-timestamp', '0');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '180');
  await page.fill('#in-accent_size', '161');
  await page.setInputFiles('#in-file', REDBLUE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('too large', { timeout: 120_000 });
  await expect(out).toHaveClass(/error/);
  await expect(page.locator('#tool-output-media')).toBeHidden();
});

// 5) Secondary input format: a WebM video also produces a valid PNG at the
//    requested canvas with the accent bar on its edge (source pixels vary, so only
//    the deterministic accent + dimensions are asserted).
test('thumbnail page accepts a WebM input and still emits a sized PNG with an accent', async ({ page }) => {
  await page.goto('/tools/youtube-thumbnail-generator/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-headline', 'WEBM');
  await page.fill('#in-timestamp', '0');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '180');
  await page.selectOption('#in-accent_position', 'bottom');
  await page.fill('#in-accent_color', '#00ff00');
  await page.fill('#in-accent_size', '40');
  await page.setInputFiles('#in-file', WEBM);

  const src = await outputSrc(page);
  const accent = await decodeAt(page, src, 160, 178);
  expect(accent.w).toBe(320);
  expect(accent.h).toBe(180);
  expect(isGreen(accent)).toBe(true);
});
