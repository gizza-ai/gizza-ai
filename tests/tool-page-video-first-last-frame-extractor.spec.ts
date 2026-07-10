import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/video-first-last-frame-extractor/ page grabs an uploaded
// video's first + last frame (@ffmpeg/core from a CDN — needs network) and
// stitches them into one image. Output is a PNG/JPG rendered as an image, so the
// media src is a data:image/... URL.
//
// Fixture redblue-64.mp4 is a 64×64 clip whose FIRST frame is solid red
// (~254,0,0) and LAST frame is solid blue (0,0,255). Both frames keep the source
// dimensions, so the stitched output is deterministic:
//   horizontal (hstack) → 128×64: red left half, blue right half
//   vertical   (vstack) → 64×128: red top half, blue bottom half

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

// Horizontal PNG: 64×64 → 128×64. Left half is the (red) first frame, right half
// is the (blue) last frame — asserted pixel-exactly, not just "an image showed".
test('first-last page stitches first|last side by side into a PNG', async ({ page }) => {
  await page.goto('/tools/video-first-last-frame-extractor/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-layout', 'horizontal');
  await page.selectOption('#in-format', 'png');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/redblue-64.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);

  const left = await decodeAt(page, src!, 16, 32);
  expect(left.w).toBe(128); // 64 + 64 side by side
  expect(left.h).toBe(64);
  // left half = first frame = red
  expect(left.r).toBeGreaterThan(220);
  expect(left.g).toBeLessThan(40);
  expect(left.b).toBeLessThan(40);

  const right = await decodeAt(page, src!, 112, 32);
  // right half = last frame = blue
  expect(right.b).toBeGreaterThan(220);
  expect(right.r).toBeLessThan(40);
  expect(right.g).toBeLessThan(40);
});

// Vertical JPG: 64×64 → 64×128, stacked. Top = first (red), bottom = last (blue).
// Also exercises JPG output (lossy, so tolerances are looser).
test('first-last page stacks first over last and honors JPG output', async ({ page }) => {
  await page.goto('/tools/video-first-last-frame-extractor/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-layout', 'vertical');
  await page.selectOption('#in-format', 'jpg');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/redblue-64.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/jpeg/);

  const top = await decodeAt(page, src!, 32, 16);
  expect(top.w).toBe(64);
  expect(top.h).toBe(128); // 64 over 64
  // top half = first frame ≈ red
  expect(top.r).toBeGreaterThan(200);
  expect(top.b).toBeLessThan(60);

  const bottom = await decodeAt(page, src!, 32, 112);
  // bottom half = last frame ≈ blue
  expect(bottom.b).toBeGreaterThan(200);
  expect(bottom.r).toBeLessThan(60);
});

// Deep-link: ?layout=vertical&format=png pre-fills the selects; uploading the
// file then runs with those values → a 64×128 stacked PNG (red top, blue bottom).
test('first-last page pre-fills from ?query and runs a stacked PNG', async ({ page }) => {
  await page.goto('/tools/video-first-last-frame-extractor/?layout=vertical&format=png');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-layout')).toHaveValue('vertical');
  await expect(page.locator('#in-format')).toHaveValue('png');
  await page.setInputFiles('#in-file', path.resolve(__dirname, 'fixtures/redblue-64.mp4'));

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);

  const top = await decodeAt(page, src!, 32, 16);
  expect(top.w).toBe(64);
  expect(top.h).toBe(128);
  expect(top.r).toBeGreaterThan(220); // red first frame on top
  expect(top.b).toBeLessThan(40);
});
