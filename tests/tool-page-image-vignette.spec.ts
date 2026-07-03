import { test, expect } from './fixtures';
import path from 'node:path';

// The generated /tools/image-vignette/ page darkens (or lightens/tints) the
// edges of an uploaded image in-browser via ffmpeg's vignette filter (plus a
// masked-merge chain for colored vignettes; single-threaded @ffmpeg/core from
// a CDN — needs network).
//
// Bounds are pre-measured against local ffmpeg 6.1 on the same fixtures:
//   white 64x64, strength 40 (default): corners ~109–116, center 255
//   white 64x64, strength 80:           corners ~2–4,     center 255
//   gray  64x64, strength 80 lighten:   corners 255,      center 128
//   white 64x64, strength 100 + tint:   corners exactly the tint color
// Margins are wide so a slightly different browser ffmpeg build still passes,
// while a silent no-op (corner staying at the input's value) still fails.

// Decode the output data URL on a canvas and sample corner + center pixels
// (full RGB triples, so tint tests can assert each channel).
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
      return { r: d[0], g: d[1], b: d[2] };
    };
    return {
      w: img.naturalWidth,
      h: img.naturalHeight,
      topLeft: px(0, 0),
      bottomRight: px(img.naturalWidth - 1, img.naturalHeight - 1),
      center: px(Math.floor(img.naturalWidth / 2), Math.floor(img.naturalHeight / 2)),
    };
  }, dataUrl);
}

async function outputSrc(page: import('@playwright/test').Page) {
  const media = page.locator('#tool-output-media');
  // ffmpeg loads from CDN on first run; allow generous time.
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\//);
  return src!;
}

test('image-vignette page darkens corners at the default strength', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  // Dimensions unchanged — a vignette never crops or resizes.
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Input is uniform white (255 everywhere). At the default strength 40 the
  // center stays bright while the corners drop to ~110 (measured 109–116).
  expect(p.center.r).toBeGreaterThan(230);
  expect(p.topLeft.r).toBeLessThan(180); // darker than the input's 255 corners
  expect(p.bottomRight.r).toBeLessThan(180);
  expect(p.topLeft.r).toBeLessThan(p.center.r - 60); // measurably darker than center
  expect(p.topLeft.r).toBeGreaterThan(40); // ...but NOT black: 40 is a soft vignette
});

test('image-vignette deep-link ?strength=80 drives a much darker vignette', async ({ page }) => {
  await page.goto('/tools/image-vignette/?strength=80');
  // The query param must land in the field before the run.
  await expect(page.locator('#in-strength')).toHaveValue('80');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Strength 80 corners are nearly black (measured 2–4) — far below the ~110
  // the default produces, so this proves the deep-linked value reached ffmpeg.
  expect(p.center.r).toBeGreaterThan(230);
  expect(p.topLeft.r).toBeLessThan(60);
  expect(p.bottomRight.r).toBeLessThan(60);
});

test('image-vignette lighten mode brightens the edges instead', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.selectOption('#in-mode', 'lighten');
  await page.fill('#in-strength', '80');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/gray-64x64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // Input is uniform mid-gray (128). Backward mode brightens with distance:
  // corners saturate to white (measured 255) while the center stays gray.
  expect(p.topLeft.r).toBeGreaterThan(230);
  expect(p.bottomRight.r).toBeGreaterThan(230);
  expect(p.center.r).toBeLessThan(160);
  expect(p.center.r).toBeGreaterThan(90);
});

test('image-vignette deep-link ?color=%23ff0000 fades the corners to red', async ({ page }) => {
  await page.goto('/tools/image-vignette/?strength=100&color=%23ff0000');
  await expect(page.locator('#in-color')).toHaveValue('#ff0000');
  await expect(page.locator('#in-strength')).toHaveValue('100');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  const p = await samplePixels(page, await outputSrc(page));
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  // At strength 100 the masked-merge chain lands the far corners exactly on
  // the tint (native ffmpeg measures (255,0,0)); the center stays white.
  expect(p.topLeft.r).toBeGreaterThan(200);
  expect(p.topLeft.g).toBeLessThan(55);
  expect(p.topLeft.b).toBeLessThan(55);
  expect(p.bottomRight.g).toBeLessThan(55);
  expect(p.center.r).toBeGreaterThan(230);
  expect(p.center.g).toBeGreaterThan(230);
  expect(p.center.b).toBeGreaterThan(230);
});

test('image-vignette bare digits-only hex color stays a color (never a number)', async ({ page }) => {
  // "112233" is numeric-looking; tool.js must NOT coerce a color field to a
  // JS number (the wasm color param is a string). Native ffmpeg measures the
  // corners at exactly (17,34,51) for this input.
  await page.goto('/tools/image-vignette/');
  await page.fill('#in-strength', '100');
  await page.fill('#in-color', '112233');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  expect(Math.abs(p.topLeft.r - 17)).toBeLessThan(30);
  expect(Math.abs(p.topLeft.g - 34)).toBeLessThan(30);
  expect(Math.abs(p.topLeft.b - 51)).toBeLessThan(30);
  expect(p.center.r).toBeGreaterThan(230);
});

test('image-vignette leading-zero all-digit hex keeps every hex digit (no Number coercion)', async ({
  page,
}) => {
  // Regression for the schema-typed coercion fix: "011733" is all decimal
  // digits, so the old sniff (and any relapse to numeric coercion) would turn
  // it into the Number 11733 → "11733", a FIVE-digit string parse_color
  // rejects → no output. A string color param must pass it through verbatim as
  // the 6-digit hex #011733 → corners (1, 23, 51). The leading zero is the
  // tell: it only survives if the value was never a JS number.
  await page.goto('/tools/image-vignette/');
  await page.fill('#in-strength', '100');
  await page.fill('#in-color', '011733');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const p = await samplePixels(page, await outputSrc(page));
  expect(Math.abs(p.topLeft.r - 1)).toBeLessThan(30); // 0x01
  expect(Math.abs(p.topLeft.g - 23)).toBeLessThan(30); // 0x17
  expect(Math.abs(p.topLeft.b - 51)).toBeLessThan(30); // 0x33
  expect(p.center.r).toBeGreaterThan(230);
});

test('image-vignette format=jpg converts the output to JPEG', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.selectOption('#in-format', 'jpg');
  await page.fill('#in-strength', '80');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/jpeg/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.jpg');
  const p = await samplePixels(page, src);
  // The vignette still applied through the conversion (wide JPEG margins).
  expect(p.w).toBe(64);
  expect(p.topLeft.r).toBeLessThan(70);
  expect(p.center.r).toBeGreaterThan(220);
});

test('image-vignette format=webp converts the output to WebP', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.selectOption('#in-format', 'webp');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const src = await outputSrc(page);
  expect(src).toMatch(/^data:image\/webp/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.webp');
  const p = await samplePixels(page, src);
  // Default strength 40 through the WebP encoder (native measures ~106).
  expect(p.w).toBe(64);
  expect(p.topLeft.r).toBeLessThan(180);
  expect(p.topLeft.r).toBeGreaterThan(40);
  expect(p.center.r).toBeGreaterThan(220);
});

test('image-vignette accepts JPEG input end-to-end', async ({ page }) => {
  await page.goto('/tools/image-vignette/?strength=80');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.jpg'));

  const src = await outputSrc(page);
  // keep (default) preserves the input format: JPEG in → JPEG out.
  expect(src).toMatch(/^data:image\/jpeg/);
  const p = await samplePixels(page, src);
  expect(p.w).toBe(64);
  expect(p.h).toBe(64);
  expect(p.topLeft.r).toBeLessThan(70); // strength 80 → near-black corners
  expect(p.center.r).toBeGreaterThan(220);
});

test('image-vignette preset chip fills the fields and mirrors the slider', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  // The "Dramatic (80)" chip sets strength; the range slider mirrors the
  // canonical number box.
  await page.getByRole('button', { name: 'Dramatic (80)' }).click();
  await expect(page.locator('#in-strength')).toHaveValue('80');
  await expect(page.locator('#in-strength-slider')).toHaveValue('80');
  // The "Sepia fade" chip drives mode + color + strength together.
  await page.getByRole('button', { name: 'Sepia fade' }).click();
  await expect(page.locator('#in-strength')).toHaveValue('70');
  await expect(page.locator('#in-color')).toHaveValue('sepia');
  await expect(page.locator('#in-mode')).toHaveValue('darken');

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  const p = await samplePixels(page, await outputSrc(page));
  // Sepia (112,66,20) at strength 70: corners pull strongly toward the tint —
  // warm (r > g > b), with blue far below the white input.
  expect(p.topLeft.r).toBeGreaterThan(p.topLeft.g + 20);
  expect(p.topLeft.g).toBeGreaterThan(p.topLeft.b + 20);
  expect(p.topLeft.b).toBeLessThan(120);
  expect(p.center.r).toBeGreaterThan(230);
});

test('image-vignette rejects a color combined with lighten mode, with guidance', async ({ page }) => {
  await page.goto('/tools/image-vignette/');
  await page.selectOption('#in-mode', 'lighten');
  await page.fill('#in-color', 'red');
  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));

  const out = page.locator('#tool-output');
  await expect(out).toHaveClass(/error/, { timeout: 90_000 });
  await expect(out).toContainText('darken'); // the error names the fix
});

// The "Copy image" button (generator template + site/tool.js): image-output
// tools get a declarative button next to Download that copies the CURRENT
// output image to the clipboard as PNG (offscreen canvas → toBlob →
// ClipboardItem, so jpg/webp/png all copy as PNG). It stays hidden until a
// result renders and never appears on video/audio/text tools.
test('copy-image button: hidden before a result, visible after, copies a PNG to the clipboard', async ({
  page,
  context,
}) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const pageErrors: string[] = [];
  page.on('pageerror', (e) => pageErrors.push(String(e)));

  await page.goto('/tools/image-vignette/');
  const copyBtn = page.locator('#tool-copy-image');
  // Rendered, but hidden while there is no output.
  await expect(copyBtn).toHaveCount(1);
  await expect(copyBtn).toBeHidden();

  await page.setInputFiles('#in-image', path.resolve(__dirname, 'fixtures/white-64x64.png'));
  // Revealed once the image result renders.
  await expect(page.locator('#tool-output-media')).toBeVisible({ timeout: 90_000 });
  await expect(copyBtn).toBeVisible();

  // The output <img> must be decoded before it can be drawn to a canvas.
  await page.waitForFunction(() => {
    const img = document.getElementById('tool-output-media') as HTMLImageElement | null;
    return !!img && img.complete && img.naturalWidth > 0;
  });

  await copyBtn.click();
  // "Copied!" affordance proves the ClipboardItem write resolved without throwing.
  await expect(copyBtn).toHaveClass(/copied/, { timeout: 10_000 });
  await expect(copyBtn).toHaveText('Copied!');

  // Permissions granted → read the clipboard back and assert a PNG is present.
  const hasPng = await page.evaluate(async () => {
    const items = await navigator.clipboard.read();
    return items.some((it) => it.types.includes('image/png'));
  });
  expect(hasPng).toBe(true);

  // Label restores after the affordance window; nothing threw on the page.
  await expect(copyBtn).toHaveText('Copy image', { timeout: 5_000 });
  expect(pageErrors).toEqual([]);
});

test('copy-image button never leaks onto text or video tool pages', async ({ page }) => {
  await page.goto('/tools/url-encode/'); // text output
  await expect(page.locator('#tool-copy-image')).toHaveCount(0);
  await page.goto('/tools/video-mute/'); // video output
  await expect(page.locator('#tool-copy-image')).toHaveCount(0);
});
