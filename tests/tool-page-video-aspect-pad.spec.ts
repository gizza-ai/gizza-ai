import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-aspect-pad/ page letterboxes/pillarboxes an
// uploaded video in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). The specs decode the OUTPUT via <video> + canvas and assert the
// media is actually correct: exact canvas dimensions for the chosen aspect,
// bar pixels in the requested color, and non-bar-colored original content in
// the center. Bounds are pre-measured with local ffmpeg on the same argv
// (bar channels land at 0/253/255 — huge margins survive codec/CSC wiggle).

const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4'); // 128×128, 2 s

/// Decode the first frame of a data:video URL and return its dimensions plus
/// the RGB values of the requested sample points.
async function decodeFrame(
  page: Page,
  dataUrl: string,
  points: Array<[number, number]>
): Promise<{ w: number; h: number; px: number[][] }> {
  return page.evaluate(
    async ({ dataUrl, points }) => {
      const v = document.createElement('video');
      v.muted = true;
      v.src = dataUrl;
      await new Promise<void>((res, rej) => {
        v.onloadeddata = () => res();
        v.onerror = () => rej(new Error('video decode failed'));
      });
      v.currentTime = 0.1;
      await new Promise<void>((res) => {
        v.onseeked = () => res();
      });
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(v, 0, 0);
      const px = points.map(([x, y]) => {
        const d = ctx.getImageData(x, y, 1, 1).data;
        return [d[0], d[1], d[2]];
      });
      return { w: v.videoWidth, h: v.videoHeight, px };
    },
    { dataUrl, points }
  );
}

test('video-aspect-pad letterboxes 128×128 to 9:16 (90×160) with red bars', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-aspect', '9:16');
  await page.fill('#in-width', '90');
  await page.fill('#in-color', '#ff0000');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);

  // Content (90×90) sits at y 35..124; bars fill y 0..34 and 125..159.
  const barPoints: Array<[number, number]> = [
    [45, 10],
    [10, 20],
    [80, 20],
    [45, 150],
    [10, 145],
  ];
  const contentPoints: Array<[number, number]> = Array.from({ length: 9 }, (_, i) => [
    10 * (i + 1) - 5,
    80,
  ]);
  const frame = await decodeFrame(page, src!, [...barPoints, ...contentPoints]);

  // Exact target canvas: 9:16 at width 90 → 90×160.
  expect(frame.w).toBe(90);
  expect(frame.h).toBe(160);

  // Bars are the requested red (measured (253,0,0) locally).
  for (const [r, g, b] of frame.px.slice(0, barPoints.length)) {
    expect(r).toBeGreaterThan(200);
    expect(g).toBeLessThan(60);
    expect(b).toBeLessThan(60);
  }
  // The center row still carries the original (non-red) content: its mean
  // green is ~112 locally, while an all-red frame would sit at ~0.
  const content = frame.px.slice(barPoints.length);
  const meanG = content.reduce((s, p) => s + p[1], 0) / content.length;
  expect(meanG).toBeGreaterThan(50);
});

test('video-aspect-pad deep link prefills and pillarboxes to 16:9 with white bars', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/?aspect=16%3A9&width=128&color=white');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-aspect')).toHaveValue('16:9', { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('128');
  await expect(page.locator('#in-color')).toHaveValue('white');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);

  // 16:9 at width 128 → 128×72; content (72×72) sits at x 28..99 with white
  // side bars (pillarbox — the other padding axis than the first test).
  const barPoints: Array<[number, number]> = [
    [5, 36],
    [14, 36],
    [22, 36],
    [105, 36],
    [120, 36],
  ];
  const contentPoints: Array<[number, number]> = Array.from({ length: 9 }, (_, i) => [
    28 + 8 * i,
    36,
  ]);
  const frame = await decodeFrame(page, src!, [...barPoints, ...contentPoints]);

  expect(frame.w).toBe(128);
  expect(frame.h).toBe(72);

  // Side bars are white (measured (255,255,255) locally).
  for (const [r, g, b] of frame.px.slice(0, barPoints.length)) {
    expect(r).toBeGreaterThan(200);
    expect(g).toBeGreaterThan(200);
    expect(b).toBeGreaterThan(200);
  }
  // The content region is NOT white canvas: the fixture has a black patch on
  // this row (min channel-sum measured 0 locally vs 765 for white).
  const content = frame.px.slice(barPoints.length);
  const minSum = Math.min(...content.map(([r, g, b]) => r + g + b));
  expect(minSum).toBeLessThan(300);
});
