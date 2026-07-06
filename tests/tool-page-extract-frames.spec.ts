import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/extract-frames/ page samples frames from an uploaded
// video (@ffmpeg/core from a CDN — needs network) and tiles them into a single
// contact-sheet image. Output is a PNG/JPG rendered as an image, so the media
// src is a data:image/... URL. Sheet dimensions are deterministic for a given
// grid + thumbnail width + source aspect (tile pads empty cells), so we can
// assert exact pixel sizes regardless of how many frames the mode sampled.
//
//   sheet_w = margin*2 + cols*thumb_w + (cols-1)*padding   (margin 4, padding 2)
//   sheet_h = margin*2 + rows*thumb_h + (rows-1)*padding

// Decode a data:image/ URL to its natural dimensions + top-left (margin) pixel.
async function decode(page: import('@playwright/test').Page, src: string) {
  return page.evaluate(async (dataUrl) => {
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
    const d = ctx.getImageData(0, 0, 1, 1).data;
    return { w: img.naturalWidth, h: img.naturalHeight, r: d[0], g: d[1], b: d[2] };
  }, src);
}

// fps mode, 128×128 mp4 → 64-px thumbs in a 2×2 grid = 138×138 PNG.
test('extract-frames page tiles fps-sampled frames into a PNG contact sheet', async ({ page }) => {
  await page.goto('/tools/extract-frames/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-mode', 'fps');
  await page.fill('#in-value', '2');
  await page.fill('#in-columns', '2');
  await page.fill('#in-rows', '2');
  await page.fill('#in-width', '64');
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const px = await decode(page, src!);
  expect(px.w).toBe(138); // 4*2 + 2*64 + 1*2
  expect(px.h).toBe(138);
});

// JPG output + a non-#-prefixed short hex path is exercised on the page too
// (the "#f00 rendered a white wave" lesson: verify the margin pixel is the
// requested color, end to end). Grid 2×2, 64-px thumbs → 138×138.
test('extract-frames page honors JPG output and a hex background color', async ({ page }) => {
  await page.goto('/tools/extract-frames/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-mode', 'fps');
  await page.fill('#in-value', '2');
  await page.fill('#in-columns', '2');
  await page.fill('#in-rows', '2');
  await page.fill('#in-width', '64');
  await page.fill('#in-background', '#ff00aa');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/jpeg/);
  const px = await decode(page, src!);
  expect(px.w).toBe(138);
  // JPEG is lossy — the margin pixel should be close to #ff00aa, not white/black.
  expect(px.r).toBeGreaterThan(220);
  expect(px.g).toBeLessThan(40);
  expect(px.b).toBeGreaterThan(120);
});

// Scene mode on a WebM (a secondary input container). Scene mode always keeps
// frame 0, so even this short clip yields a padded sheet. 64×64 → 32-px thumbs
// in a 2×2 grid = 74×74; the black background fills the margins/padding.
test('extract-frames page handles scene mode on a webm input', async ({ page }) => {
  await page.goto('/tools/extract-frames/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-mode', 'scene');
  await page.fill('#in-value', '0.1');
  await page.fill('#in-columns', '2');
  await page.fill('#in-rows', '2');
  await page.fill('#in-width', '32');
  await page.fill('#in-background', 'black');
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/clip-1s.webm'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const px = await decode(page, src!);
  expect(px.w).toBe(74); // 4*2 + 2*32 + 1*2
  expect(px.h).toBe(74);
  // black margin pixel
  expect(px.r).toBeLessThan(24);
  expect(px.g).toBeLessThan(24);
  expect(px.b).toBeLessThan(24);
});

// Deep-link: the query string pre-fills the fields (interval mode, 3×2 grid,
// 64-px thumbs). Uploading the file then runs with those values → 204×138.
test('extract-frames page pre-fills from ?query and runs an interval sheet', async ({ page }) => {
  await page.goto('/tools/extract-frames/?mode=interval&value=2&columns=3&rows=2&width=64&format=png');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-columns')).toHaveValue('3');
  await expect(page.locator('#in-rows')).toHaveValue('2');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const px = await decode(page, src!);
  expect(px.w).toBe(204); // 4*2 + 3*64 + 2*2
  expect(px.h).toBe(138); // 4*2 + 2*64 + 1*2
});
