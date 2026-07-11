import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-waveform-video/ page renders an ANIMATED waveform
// MP4 from an uploaded audio file in-browser via ffmpeg's showwaves (@ffmpeg/core
// from jsDelivr — needs network). Mixed media shape: audio in, VIDEO out
// (out.mp4 → data:video/mp4). Beyond the mime prefix, each test decodes a real
// frame on a canvas and asserts the REQUESTED dimensions and real wave pixels
// (media-correctness rule): an MP4 has no alpha, so the background is always
// opaque; the default background is a near-black slate with an indigo wave, and
// a red run must draw red wave pixels.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');
const STEREO = path.resolve(__dirname, 'fixtures/tone-stereo-3s.mp3');
const WAV = path.resolve(__dirname, 'fixtures/tone-3s.wav');

type Stats = {
  w: number;
  h: number;
  corner: number[]; // RGBA at (0,0)
  indigo: number; // pixels matching the default accent wave (#4f46e5)
  red: number; // pixels matching a red wave
  leftRed: number; // red pixels in the left quarter (gradient start)
  rightBlue: number; // blue pixels in the right quarter (gradient end)
};

// Seek a data:video URL to `time` seconds, draw that frame to a canvas, and
// count pixels matching a few fixed color predicates. Antialiasing blends edges,
// so match generously on the dominant channel.
async function frameStats(page: Page, dataUrl: string, time: number): Promise<Stats> {
  return page.evaluate(
    async ({ dataUrl, time }) => {
      const v = document.createElement('video');
      v.muted = true;
      v.src = dataUrl;
      await new Promise<void>((res, rej) => {
        v.onloadeddata = () => res();
        v.onerror = () => rej(new Error('video decode failed'));
      });
      v.currentTime = time;
      await new Promise<void>((res) => {
        v.onseeked = () => res();
      });
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(v, 0, 0);
      const d = ctx.getImageData(0, 0, c.width, c.height).data;
      const s = {
        w: v.videoWidth,
        h: v.videoHeight,
        corner: [d[0], d[1], d[2], d[3]],
        indigo: 0,
        red: 0,
        leftRed: 0,
        rightBlue: 0,
      };
      for (let y = 0; y < c.height; y++) {
        for (let x = 0; x < c.width; x++) {
          const i = (y * c.width + x) * 4;
          const r = d[i], g = d[i + 1], b = d[i + 2];
          if (b > 120 && b > r && b > g && r < 160) s.indigo++;
          if (r > 150 && r > b && g < 110) s.red++;
          if (x < c.width / 4 && r > 150 && b < 110) s.leftRed++;
          if (x >= (3 * c.width) / 4 && b > 150 && r < 110) s.rightBlue++;
        }
      }
      return s;
    },
    { dataUrl, time },
  );
}

async function outputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
  return src!;
}

// 1. Core correctness: the default MIRROR style renders the REQUESTED size, an
//    OPAQUE near-black background (MP4 has no alpha), and a real indigo wave.
//    Long-hex #101014 default background is exercised implicitly.
test('audio-waveform-video renders a 480x180 mirror clip with an opaque slate bg and indigo wave', async ({
  page,
}) => {
  await page.goto('/tools/audio-waveform-video/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '480');
  await page.fill('#in-height', '180');
  await page.setInputFiles('#in-file', FIXTURE);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.5);
  expect(f.w).toBe(480); // the REQUESTED size, not the 1280 default
  expect(f.h).toBe(180);
  expect(f.corner[3]).toBe(255); // video is always opaque
  expect(f.corner[0]).toBeLessThan(40); // near-black slate #101014
  expect(f.corner[1]).toBeLessThan(40);
  expect(f.corner[2]).toBeLessThan(50);
  expect(f.indigo).toBeGreaterThan(80); // a real #4f46e5 wave was drawn
});

// 2. Deep link prefills every field; BARS mode, a SHORT-hex color (#f00 — the
//    regression that once drew a white wave), sqrt scale and fps=30 must all take
//    effect, and the color swatch mirrors the deep-linked hex.
test('audio-waveform-video deep link renders short-hex red bars, sqrt-boosted', async ({ page }) => {
  await page.goto(
    '/tools/audio-waveform-video/?mode=bars&width=480&height=180&color=%23f00&scale=sqrt&fps=30',
  );
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-mode')).toHaveValue('bars', { timeout: 15_000 });
  await expect(page.locator('#in-width')).toHaveValue('480');
  await expect(page.locator('#in-height')).toHaveValue('180');
  await expect(page.locator('#in-color')).toHaveValue('#f00');
  await expect(page.locator('#in-scale')).toHaveValue('sqrt');
  await expect(page.locator('#in-fps')).toHaveValue('30');
  // The color swatch (kind="color") expands the short hex to the long form.
  await expect(page.locator('#in-color-swatch')).toHaveValue('#ff0000');
  await page.setInputFiles('#in-file', FIXTURE);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.5);
  expect(f.w).toBe(480);
  expect(f.h).toBe(180);
  expect(f.red).toBeGreaterThan(80); // short-hex #f00 → real red, not white
});

// 3. Gradient (color2): red on the left fading to blue on the right — also proves
//    the browser ffmpeg build supports the gradients/alphaextract/alphamerge chain.
test('audio-waveform-video gradient wave fades red-left to blue-right', async ({ page }) => {
  await page.goto('/tools/audio-waveform-video/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '480');
  await page.fill('#in-height', '180');
  await page.fill('#in-color', '#ff0000');
  await page.fill('#in-color2', '#0000ff');
  await page.setInputFiles('#in-file', FIXTURE);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.5);
  expect(f.w).toBe(480);
  expect(f.leftRed).toBeGreaterThan(20); // gradient start
  expect(f.rightBlue).toBeGreaterThan(20); // gradient end
});

// 4. Remaining draw modes (wave, points) each render a real wave — one run per
//    enum choice not covered above (mirror = test 1, bars = test 2).
for (const mode of ['wave', 'points'] as const) {
  test(`audio-waveform-video ${mode} mode draws a visible indigo wave`, async ({ page }) => {
    await page.goto('/tools/audio-waveform-video/');
    await page.waitForSelector('#in-file');
    await page.selectOption('#in-mode', mode);
    await page.fill('#in-width', '480');
    await page.fill('#in-height', '180');
    await page.setInputFiles('#in-file', FIXTURE);
    const src = await outputSrc(page);
    const f = await frameStats(page, src, 1.5);
    expect(f.w).toBe(480);
    // showwaves p2p/point modes draw fewer pixels than mirror/bars on this
    // short sine fixture; still require clearly non-background indigo pixels.
    expect(f.indigo).toBeGreaterThan(10);
  });
}

// 5. Remaining amplitude scales (cbrt, log) are accepted and render (lin = test 1,
//    sqrt = test 2). Quiet-audio boost keeps a wave on screen.
for (const scale of ['cbrt', 'log'] as const) {
  test(`audio-waveform-video ${scale} scale is accepted and renders`, async ({ page }) => {
    await page.goto('/tools/audio-waveform-video/');
    await page.waitForSelector('#in-file');
    await page.selectOption('#in-scale', scale);
    await page.fill('#in-width', '480');
    await page.fill('#in-height', '180');
    await page.setInputFiles('#in-file', FIXTURE);
    const src = await outputSrc(page);
    const f = await frameStats(page, src, 1.5);
    expect(f.indigo).toBeGreaterThan(20);
  });
}

// 6. Secondary input format: a WAV (PCM) is accepted, and the exact fps CAP (60)
//    still renders. Stereo is downmixed to a single mono wave. One run covers a
//    non-mp3 container + the boundary + stereo→mono.
test('audio-waveform-video accepts a WAV at the fps cap and downmixes stereo', async ({ page }) => {
  await page.goto('/tools/audio-waveform-video/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-width', '480');
  await page.fill('#in-height', '180');
  await page.fill('#in-fps', '60'); // exact cap → accepted
  await page.setInputFiles('#in-file', WAV);
  const wavSrc = await outputSrc(page);
  const w = await frameStats(page, wavSrc, 1.5);
  expect(w.w).toBe(480);
  expect(w.indigo).toBeGreaterThan(20);

  // A stereo mp3 renders too (downmixed to one wave — not a decode error).
  await page.setInputFiles('#in-file', STEREO);
  await expect
    .poll(async () => (await page.locator('#tool-output-media').getAttribute('src')) !== wavSrc, {
      timeout: 120_000,
    })
    .toBe(true);
  const s = await frameStats(page, (await page.locator('#tool-output-media').getAttribute('src'))!, 1.5);
  expect(s.indigo).toBeGreaterThan(20);
});

// 7. Bad hex is rejected with the guiding error, and the static page contract:
//    labelled style options and the three example chips.
test('audio-waveform-video rejects a named color and ships preset chips', async ({ page }) => {
  await page.goto('/tools/audio-waveform-video/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-color', 'red');
  await page.setInputFiles('#in-file', FIXTURE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('hex color', { timeout: 120_000 });
  await expect(out).toContainText('#4f46e5');

  await expect(page.locator('#in-mode option[value="mirror"]')).toHaveText(
    'Mirror — centered, symmetric (default)',
  );
  await expect(page.locator('.tool-example-chip')).toHaveCount(3);
});
