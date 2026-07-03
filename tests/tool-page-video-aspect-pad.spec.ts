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
  await page.goto('/tools/video-aspect-pad/?aspect=16%3A9&width=128&color=white&quality=high');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-aspect')).toHaveValue('16:9', { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('128');
  await expect(page.locator('#in-color')).toHaveValue('white');
  await expect(page.locator('#in-quality')).toHaveValue('high');
  await expect(page.locator('#in-blur')).not.toBeChecked();
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

// Blurred-background fill: the "Blurred background 9:16" example chip checks
// the blur box and sets the aspect; the bars must then carry blurred CONTENT
// (colorful, non-uniform) instead of a solid color. Bounds pre-measured with
// local ffmpeg on the same argv: bar channel-sums 310-462 (solid black would
// be ~0, white ~765), red channel varies by ~80 across the bar.
test('video-aspect-pad blur chip fills the bars with blurred video, not a solid color', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '90');
  await page.click('.tool-example-chip:has-text("Blurred background 9:16")');
  await expect(page.locator('#in-blur')).toBeChecked();
  await expect(page.locator('#in-aspect')).toHaveValue('9:16');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);

  // Content (90×90) sits at y 35..124; bars fill y 0..34 and 125..159.
  const barPoints: Array<[number, number]> = [
    [10, 10],
    [25, 10],
    [45, 10],
    [65, 10],
    [80, 10],
    [10, 150],
    [45, 150],
    [80, 150],
  ];
  const contentPoints: Array<[number, number]> = [
    [5, 80], // white stripe (sum ~765 locally)
    [85, 80], // black stripe (sum ~1 locally)
  ];
  const frame = await decodeFrame(page, src!, [...barPoints, ...contentPoints]);
  expect(frame.w).toBe(90);
  expect(frame.h).toBe(160);

  // Bars are blurred footage: mid-brightness, and varied — not one flat color.
  const bars = frame.px.slice(0, barPoints.length);
  for (const [r, g, b] of bars) {
    const sum = r + g + b;
    expect(sum).toBeGreaterThan(100); // not solid black
    expect(sum).toBeLessThan(700); // not solid white
  }
  const sums = bars.map(([r, g, b]) => r + g + b);
  expect(Math.max(...sums) - Math.min(...sums)).toBeGreaterThan(30); // non-uniform
  // The foreground content is still crisp on top (white and black stripes).
  expect(frame.px[barPoints.length].reduce((a, b) => a + b, 0)).toBeGreaterThan(600);
  expect(frame.px[barPoints.length + 1].reduce((a, b) => a + b, 0)).toBeLessThan(150);
});

// Short-hex color + the new 3:2 preset, end-to-end via deep link: #f0f must
// render true magenta bars (a broken hex path would fall back to black), and
// 3:2 at width 90 must yield exactly 90×60 (content 60 wide, 15px side bars).
// Bounds pre-measured locally: bars ≈ (254, 0, 253).
test('video-aspect-pad deep link 3:2 with #f0f renders magenta pillarbox bars', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/?aspect=3%3A2&width=90&color=%23f0f');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-aspect')).toHaveValue('3:2', { timeout: 15_000 });
  await expect(page.locator('#in-color')).toHaveValue('#f0f');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');

  // Content (60×60) sits at x 15..74; bars fill x 0..14 and 75..89.
  const barPoints: Array<[number, number]> = [
    [3, 30],
    [7, 30],
    [11, 30],
    [79, 30],
    [83, 30],
    [87, 30],
  ];
  const contentPoints: Array<[number, number]> = [
    [20, 30],
    [35, 30],
    [45, 30],
    [55, 30],
    [70, 30],
  ];
  const frame = await decodeFrame(page, src!, [...barPoints, ...contentPoints]);
  expect(frame.w).toBe(90);
  expect(frame.h).toBe(60);
  for (const [r, g, b] of frame.px.slice(0, barPoints.length)) {
    expect(r).toBeGreaterThan(200);
    expect(g).toBeLessThan(60);
    expect(b).toBeGreaterThan(200);
  }
  // Content row carries the fixture's stripes, not magenta: mean green ~142
  // (white/yellow/green stripes) and a low-blue stripe (yellow b≈31).
  const content = frame.px.slice(barPoints.length);
  const meanG = content.reduce((s, p) => s + p[1], 0) / content.length;
  expect(meanG).toBeGreaterThan(80);
  expect(Math.min(...content.map((p) => p[2]))).toBeLessThan(100);
});

// Paste-to-upload (shared tool.js): a synthetic paste event carrying a video
// file must select it and run — same output geometry as choosing the file.
test('video-aspect-pad paste-to-upload runs the conversion', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '90');
  const b64 = require('node:fs').readFileSync(FIXTURE).toString('base64');
  await page.evaluate(async (b64: string) => {
    const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    const dt = new DataTransfer();
    dt.items.add(new File([bytes], 'pasted.mp4', { type: 'video/mp4' }));
    document.dispatchEvent(new ClipboardEvent('paste', { clipboardData: dt, bubbles: true, cancelable: true }));
  }, b64);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  const frame = await decodeFrame(page, src!, [[45, 10]]);
  expect(frame.w).toBe(90); // default 9:16 at width 90 → 90×160
  expect(frame.h).toBe(160);
  const [r, g, b] = frame.px[0];
  expect(r + g + b).toBeLessThan(100); // default black bars
});

// Static page contract: the generated CLI example must be copy-paste-runnable
// (no placeholder text leaking into a param value), and the aspect presets
// must be platform-labeled while keeping canonical option values.
test('video-aspect-pad page ships a runnable CLI example and labeled presets', async ({ page }) => {
  await page.goto('/tools/video-aspect-pad/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toBe(
    "gizza tool video-aspect-pad 'url=https://example.com/input' 'aspect=9:16' 'blur=true' 'quality=medium'"
  );
  const opt = page.locator('#in-aspect option[value="9:16"]');
  await expect(opt).toHaveText('9:16 — Reels / Shorts / TikTok (1080×1920)');
  await expect(page.locator('#in-aspect option[value="3:2"]')).toHaveText('3:2 — classic photo (1620×1080)');
  await expect(page.locator('#tool-reset')).toBeVisible();
  await expect(page.locator('.tool-example-chip')).toHaveCount(5);
});
