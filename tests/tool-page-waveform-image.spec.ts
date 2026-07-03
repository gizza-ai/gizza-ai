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
