import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-drop-shadow/ page casts a shadow from the input's
// ALPHA channel via ffmpeg (single-threaded @ffmpeg/core from a CDN — needs
// network), then grows the canvas so the shadow is never clipped.
//
// Every bound below is pre-measured against local ffmpeg 7.1 running the exact
// filtergraph core::plan builds, on these fixtures:
//   alpha-grad-64.png — 64x64 red, alpha ramping 0 (left) -> 255 (right)
//   white-64x64.png   — 64x64 fully opaque white (no alpha -> boxy shadow)
// Margins are wide enough for a different browser ffmpeg/encoder build, but a
// no-op (unchanged dimensions, or a missing/rectangular shadow) still fails.

type Px = { r: number; g: number; b: number; a: number };

// Decode the output data URL onto a canvas and sample RGBA at given points.
async function samplePixels(
  page: import('@playwright/test').Page,
  dataUrl: string,
  points: [number, number][],
): Promise<{ w: number; h: number; px: Px[] }> {
  return page.evaluate(
    async ({ src, points }) => {
      const img = new Image();
      await new Promise((res, rej) => {
        img.onload = res;
        img.onerror = rej;
        img.src = src;
      });
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      return {
        w: img.naturalWidth,
        h: img.naturalHeight,
        px: points.map(([x, y]) => {
          const d = ctx.getImageData(x, y, 1, 1).data;
          return { r: d[0], g: d[1], b: d[2], a: d[3] };
        }),
      };
    },
    { src: dataUrl, points },
  );
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  // ffmpeg loads from CDN on first run; allow generous time.
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

test('image-drop-shadow grows the canvas and casts the shadow from the alpha channel', async ({
  page,
}) => {
  await page.goto('/tools/image-drop-shadow/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/alpha-grad-64.png'));

  // Sample: both far corners, the shadow band under the OPAQUE right half,
  // the same band under the TRANSPARENT left half, a canvas point left of the
  // subject, and the opaque subject itself.
  const { w, h, px } = await samplePixels(page, await outputSrc(page), [
    [0, 0],
    [167, 167],
    [120, 125],
    [70, 125],
    [60, 84],
    [115, 84],
  ]);
  const [topLeft, bottomRight, underOpaque, underTransparent, leftOfSubject, subject] = px;

  // Defaults offset 12/16 + blur 24 -> auto margin 52 per side: 64 -> 168.
  expect(w).toBe(168);
  expect(h).toBe(168);
  // The blurred shadow never reaches the frame — both far corners stay clear.
  expect(topLeft.a).toBeLessThan(10);
  expect(bottomRight.a).toBeLessThan(10);
  // Shadow under the opaque half (native measures alpha 45) is black, not red.
  expect(underOpaque.a).toBeGreaterThan(20);
  expect(underOpaque.a).toBeLessThan(110);
  expect(underOpaque.r).toBeLessThan(60);
  // ...and it FOLLOWS the alpha: under the transparent half there is almost
  // none (native 3), and beside the subject's transparent edge none at all
  // (native 1). A rectangular box-shadow would light both of these up.
  expect(underTransparent.a).toBeLessThan(15);
  expect(leftOfSubject.a).toBeLessThan(15);
  expect(underOpaque.a).toBeGreaterThan(underTransparent.a + 15);
  // The subject is composited back on top untouched: opaque red.
  expect(subject.a).toBe(255);
  expect(subject.r).toBeGreaterThan(200);
  expect(subject.g).toBeLessThan(55);
});

test('image-drop-shadow deep-link ?blur=0&opacity=100&offset_x=20&offset_y=20 gives a hard sticker shadow', async ({
  page,
}) => {
  await page.goto(
    '/tools/image-drop-shadow/?blur=0&opacity=100&offset_x=20&offset_y=20',
  );
  // The query params must land in the fields before the run.
  await expect(page.locator('#in-blur')).toHaveValue('0');
  await expect(page.locator('#in-opacity')).toHaveValue('100');
  await expect(page.locator('#in-offset_x')).toHaveValue('20');
  await expect(page.locator('#in-offset_y')).toHaveValue('20');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  const { w, h, px } = await samplePixels(page, await outputSrc(page), [
    [5, 5],
    [30, 30],
    [95, 95],
    [90, 50],
  ]);
  const [corner, subject, shadowCorner, shadowRight] = px;

  // blur 0 -> no Gaussian reach, so the auto margin is purely the 20 px
  // offset: 64 -> 104. Different from the default 168, which proves the
  // deep-linked values reached the wasm plan and not just the input boxes.
  expect(w).toBe(104);
  expect(h).toBe(104);
  // Above-left of the subject: outside the shifted silhouette, still clear.
  expect(corner.a).toBeLessThan(10);
  // The opaque white subject.
  expect(subject.a).toBe(255);
  expect(subject.r).toBeGreaterThan(230);
  // Hard edge at opacity 100 -> fully solid black shadow, no soft falloff.
  expect(shadowCorner.a).toBeGreaterThan(240);
  expect(shadowCorner.r).toBeLessThan(20);
  expect(shadowRight.a).toBeGreaterThan(240);
  expect(shadowRight.r).toBeLessThan(20);
});

test('image-drop-shadow format=jpg flattens onto white and honours color=red', async ({ page }) => {
  await page.goto('/tools/image-drop-shadow/');
  await page.selectOption('#in-format', 'jpg');
  await page.fill('#in-color', 'red');
  await page.fill('#in-blur', '0');
  await page.fill('#in-opacity', '100');
  await page.fill('#in-offset_x', '20');
  await page.fill('#in-offset_y', '20');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');

  const { w, h, px } = await samplePixels(page, src, [
    [5, 5],
    [95, 95],
    [90, 50],
  ]);
  const [corner, shadowCorner, shadowRight] = px;
  expect(w).toBe(104);
  expect(h).toBe(104);
  // JPEG cannot store alpha: the transparent canvas is flattened onto white,
  // so every pixel is opaque and the margin reads as solid white.
  expect(corner.a).toBe(255);
  expect(corner.r).toBeGreaterThan(230);
  expect(corner.g).toBeGreaterThan(230);
  // The shadow is the requested red, not black (native measures 254,0,0).
  expect(shadowCorner.r).toBeGreaterThan(200);
  expect(shadowCorner.g).toBeLessThan(55);
  expect(shadowCorner.b).toBeLessThan(55);
  expect(shadowRight.r).toBeGreaterThan(200);
  expect(shadowRight.g).toBeLessThan(55);
});

test('image-drop-shadow format=webp + keep-original-size holds the input dimensions', async ({
  page,
}) => {
  await page.goto('/tools/image-drop-shadow/');
  await page.selectOption('#in-format', 'webp');
  await page.check('#in-clip_to_original');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/alpha-grad-64.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/webp/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.webp');

  const { w, h, px } = await samplePixels(page, src, [
    [0, 0],
    [63, 63],
  ]);
  const [topLeft, subject] = px;
  // clip_to_original overrides the auto margin entirely: 64x64 in, 64x64 out
  // (the default run on this same fixture grows to 168x168).
  expect(w).toBe(64);
  expect(h).toBe(64);
  // WebP kept the alpha through the encode: the transparent corner is still
  // transparent, the opaque corner still solid red.
  expect(topLeft.a).toBeLessThan(15);
  expect(subject.a).toBeGreaterThan(240);
  expect(subject.r).toBeGreaterThan(200);
});
