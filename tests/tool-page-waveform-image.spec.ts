import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/waveform-image/ page renders a static waveform PNG from
// an uploaded audio file in-browser via ffmpeg's showwavespic (@ffmpeg/core
// from jsDelivr — needs network). Mixed media shape: audio in, image out
// (out.png → data:image/png). Beyond the mime prefix, each test decodes the
// image on a canvas and asserts the REQUESTED dimensions and real wave pixels
// (media-correctness rule): the default background must be transparent at the
// corners with colored wave pixels in the body; a background run must have
// opaque background corners.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');
const STEREO_FIXTURE = path.resolve(__dirname, 'fixtures/tone-stereo-3s.mp3');

type Decoded = {
  w: number;
  h: number;
  corner: number[]; // RGBA at (0,0)
  wavePx: number; // pixels matching the expected wave color
  opaquePx: number; // pixels with alpha > 0
};

// Decode the produced PNG and count wave-colored pixels. `waveTest` is the
// name of a per-pixel predicate evaluated in the page.
async function decodePng(page: Page, src: string, wave: 'indigo' | 'red'): Promise<Decoded> {
  return page.evaluate(
    async ({ dataUrl, wave }) => {
      const img = new Image();
      await new Promise<void>((res, rej) => {
        img.onload = () => res();
        img.onerror = rej;
        img.src = dataUrl;
      });
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(img, 0, 0);
      const d = ctx.getImageData(0, 0, c.width, c.height).data;
      let wavePx = 0;
      let opaquePx = 0;
      for (let i = 0; i < d.length; i += 4) {
        const [r, g, b, a] = [d[i], d[i + 1], d[i + 2], d[i + 3]];
        if (a > 0) opaquePx++;
        // Antialiasing blends edges, so match generously on the dominant channel.
        const isWave =
          wave === 'indigo' ? a > 0 && b > 150 && b > r : a > 0 && r > 150 && r > b && g < 100;
        if (isWave) wavePx++;
      }
      return {
        w: img.naturalWidth,
        h: img.naturalHeight,
        corner: [d[0], d[1], d[2], d[3]],
        wavePx,
        opaquePx,
      };
    },
    { dataUrl: src, wave },
  );
}

test('waveform-image renders a transparent 320x100 PNG with accent wave pixels', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const png = await decodePng(page, src!, 'indigo');
  expect(png.w).toBe(320); // the REQUESTED size, not the default
  expect(png.h).toBe(100);
  expect(png.corner[3]).toBe(0); // default background is transparent
  expect(png.wavePx).toBeGreaterThan(100); // a real wave was drawn (#4f46e5)
});

test('waveform-image background run yields opaque black corners and a red wave', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.fill('#in-color', '#ff0000');
  await page.fill('#in-background', '#000000');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const png = await decodePng(page, src!, 'red');
  expect(png.w).toBe(320);
  expect(png.h).toBe(100);
  // Opaque black background corner…
  expect(png.corner[3]).toBe(255);
  expect(png.corner[0]).toBeLessThan(10);
  expect(png.corner[1]).toBeLessThan(10);
  expect(png.corner[2]).toBeLessThan(10);
  // …every pixel painted (no alpha holes), and a red wave on top.
  expect(png.opaquePx).toBe(320 * 100);
  expect(png.wavePx).toBeGreaterThan(100);
});

test('waveform-image rejects a named color with the guiding hex error', async ({ page }) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-color', 'red');
  await page.setInputFiles('#in-audio', FIXTURE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('hex color', { timeout: 90_000 });
  await expect(out).toContainText('#4f46e5');
});

test('waveform-image deep link prefills size, color and scale, then renders', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/?width=320&height=100&color=%23ff0000&scale=sqrt');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-width')).toHaveValue('320', { timeout: 15_000 });
  await expect(page.locator('#in-height')).toHaveValue('100');
  await expect(page.locator('#in-color')).toHaveValue('#ff0000');
  await expect(page.locator('#in-scale')).toHaveValue('sqrt');
  // The color swatch (kind="color") mirrors the deep-linked text value.
  await expect(page.locator('#in-color-swatch')).toHaveValue('#ff0000');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:image\/png/);
  const png = await decodePng(page, src!, 'red');
  expect(png.w).toBe(320);
  expect(png.h).toBe(100);
  expect(png.corner[3]).toBe(0); // still transparent (no background set)
  expect(png.wavePx).toBeGreaterThan(100); // red wave, sqrt-boosted
});

// REGRESSION: a 3-digit hex used to reach ffmpeg raw, which warns and
// silently draws a WHITE wave. Core now expands #f00 → #ff0000.
test('waveform-image renders a 3-digit hex as the actual color, not white', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.fill('#in-color', '#f00');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const png = await decodePng(page, (await media.getAttribute('src'))!, 'red');
  expect(png.wavePx).toBeGreaterThan(100); // red wave — the bug drew 0 red px
});

// Gradient wave (color2): also proves the browser ffmpeg build supports the
// gradients/alphaextract/alphamerge chain the recipe uses.
test('waveform-image gradient run fades red on the left to blue on the right', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.fill('#in-color', '#ff0000');
  await page.fill('#in-color2', '#0000ff');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  const sides = await page.evaluate(async (dataUrl) => {
    const img = new Image();
    await new Promise<void>((res, rej) => {
      img.onload = () => res();
      img.onerror = rej;
      img.src = dataUrl;
    });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let leftRed = 0;
    let rightBlue = 0;
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        const i = (y * c.width + x) * 4;
        const [r, g, b, a] = [d[i], d[i + 1], d[i + 2], d[i + 3]];
        if (a > 0 && x < c.width / 4 && r > 150 && b < 110) leftRed++;
        if (a > 0 && x >= (3 * c.width) / 4 && b > 150 && r < 110) rightBlue++;
      }
    }
    return { w: c.width, h: c.height, corner: d[3], leftRed, rightBlue };
  }, src!);
  expect(sides.w).toBe(320);
  expect(sides.h).toBe(100);
  expect(sides.corner).toBe(0); // background still transparent
  expect(sides.leftRed).toBeGreaterThan(50); // gradient start
  expect(sides.rightBlue).toBeGreaterThan(50); // gradient end
});

// Per-channel colors: a stereo file with split lanes and a comma list draws
// the left channel red (top lane) and the right channel blue (bottom lane).
test('waveform-image stereo split lanes take per-channel colors', async ({ page }) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.fill('#in-color', '#ff0000,#0000ff');
  await page.check('#in-split_channels');
  await page.setInputFiles('#in-audio', STEREO_FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  const lanes = await page.evaluate(async (dataUrl) => {
    const img = new Image();
    await new Promise<void>((res, rej) => {
      img.onload = () => res();
      img.onerror = rej;
      img.src = dataUrl;
    });
    const c = document.createElement('canvas');
    c.width = img.naturalWidth;
    c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(img, 0, 0);
    const d = ctx.getImageData(0, 0, c.width, c.height).data;
    let topRed = 0;
    let bottomBlue = 0;
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        const i = (y * c.width + x) * 4;
        const [r, , b, a] = [d[i], d[i + 1], d[i + 2], d[i + 3]];
        if (a > 0 && y < c.height / 2 && r > 150 && b < 100) topRed++;
        if (a > 0 && y >= c.height / 2 && b > 150 && r < 100) bottomBlue++;
      }
    }
    return { topRed, bottomBlue };
  }, src!);
  expect(lanes.topRed).toBeGreaterThan(100);
  expect(lanes.bottomBlue).toBeGreaterThan(100);
});

// sampling=peak draws the loudest sample per column — strictly more wave
// pixels than the default average on the same input.
test('waveform-image peak sampling draws a fuller wave than average', async ({ page }) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const avgSrc = (await media.getAttribute('src'))!;
  const avg = await decodePng(page, avgSrc, 'indigo');
  await page.selectOption('#in-sampling', 'peak');
  // The change re-runs ffmpeg; wait for the media src to change.
  await expect
    .poll(async () => await media.getAttribute('src'), { timeout: 90_000 })
    .not.toBe(avgSrc);
  const peak = await decodePng(page, (await media.getAttribute('src'))!, 'indigo');
  expect(peak.wavePx).toBeGreaterThan(avg.wavePx);
});

// The "Sunset gradient" example chip pre-fills the gradient + background and
// re-renders; the color swatches mirror the chip's hex values.
test('waveform-image example chip applies a gradient preset and re-renders', async ({
  page,
}) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  await page.click('.tool-example-chip:has-text("Sunset gradient")');
  await expect(page.locator('#in-color')).toHaveValue('#f97316');
  await expect(page.locator('#in-color2')).toHaveValue('#ec4899');
  await expect(page.locator('#in-background')).toHaveValue('#0b1220');
  await expect(page.locator('#in-color-swatch')).toHaveValue('#f97316');
  await expect(page.locator('#in-color2-swatch')).toHaveValue('#ec4899');
  await expect(media).toBeVisible({ timeout: 90_000 });
  await expect
    .poll(
      async () => {
        const png = await decodePng(page, (await media.getAttribute('src'))!, 'red');
        return png.corner[3];
      },
      { timeout: 90_000 },
    )
    .toBe(255); // the chip's #0b1220 background is opaque
});

// The native swatch is a two-way mirror: picking a color writes the hex into
// the canonical text input (which the run reads).
test('waveform-image color swatch pick updates the hex field and runs', async ({ page }) => {
  await page.goto('/tools/waveform-image/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-width', '320');
  await page.fill('#in-height', '100');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  // Drive the swatch the way a user's picker-close does (input + change).
  await page.evaluate(() => {
    const s = document.getElementById('in-color-swatch') as HTMLInputElement;
    s.value = '#ff0000';
    s.dispatchEvent(new Event('input'));
    s.dispatchEvent(new Event('change'));
  });
  await expect(page.locator('#in-color')).toHaveValue('#ff0000');
  await expect
    .poll(
      async () => {
        const png = await decodePng(page, (await media.getAttribute('src'))!, 'red');
        return png.wavePx;
      },
      { timeout: 90_000 },
    )
    .toBeGreaterThan(100); // re-ran with the picked red
});
