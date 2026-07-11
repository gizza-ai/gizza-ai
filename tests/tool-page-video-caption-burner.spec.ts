import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const BLACK = path.resolve(__dirname, 'fixtures/title-black-3s.mp4'); // 240×180, 3 s, black
const WEBM = path.resolve(__dirname, 'fixtures/clip-1s.webm');

const SRT = `1
00:00:01,000 --> 00:00:02,000
HELLO

2
00:00:02,000 --> 00:00:03,000
BYE`;

async function outputSrc(page: Page): Promise<string> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 120_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\//);
  return src!;
}

async function frameStats(page: Page, dataUrl: string, time: number) {
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
      await new Promise<void>((res) => { v.onseeked = () => res(); });
      const c = document.createElement('canvas');
      c.width = v.videoWidth;
      c.height = v.videoHeight;
      const ctx = c.getContext('2d')!;
      ctx.drawImage(v, 0, 0);
      const { data } = ctx.getImageData(0, 0, c.width, c.height);
      const mid = c.height / 2;
      const s = {
        w: v.videoWidth,
        h: v.videoHeight,
        white: 0,
        whiteTop: 0,
        whiteBottom: 0,
        yellow: 0,
        yellowTop: 0,
        yellowBottom: 0,
        navy: 0,
        navyTop: 0,
        navyBottom: 0,
        nonBlack: 0,
      };
      for (let i = 0; i < data.length; i += 4) {
        const r = data[i], g = data[i + 1], b = data[i + 2];
        const row = ((i / 4) / c.width) | 0;
        const top = row < mid;
        if (r > 40 || g > 40 || b > 40) s.nonBlack++;
        if (r > 180 && g > 180 && b > 180) { s.white++; top ? s.whiteTop++ : s.whiteBottom++; }
        if (r > 170 && g > 170 && b < 110) { s.yellow++; top ? s.yellowTop++ : s.yellowBottom++; }
        if (b > 50 && r < 90 && g < 90) { s.navy++; top ? s.navyTop++ : s.navyBottom++; }
      }
      return s;
    },
    { dataUrl, time }
  );
}

test('video-caption-burner burns timed white subtitles only inside cue windows', async ({ page }) => {
  await page.goto('/tools/video-caption-burner/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-subtitles', SRT);
  await page.selectOption('#in-position', 'bottom');
  await page.fill('#in-font_color', '#ffffff');
  await page.uncheck('#in-background'); // non-default checkbox state
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);
  const inCue = await frameStats(page, src, 1.5);
  expect(inCue.w).toBe(240);
  expect(inCue.h).toBe(180);
  expect(inCue.whiteBottom).toBeGreaterThan(20);

  const beforeCue = await frameStats(page, src, 0.3);
  expect(beforeCue.white).toBeLessThan(5);
  expect(beforeCue.nonBlack).toBeLessThan(20);
});

test('video-caption-burner deep link renders short-hex yellow captions at the top', async ({ page }) => {
  const params = new URLSearchParams({
    position: 'top',
    font_color: '#ff0',
    background: 'false',
    font_size: '32',
  });
  await page.goto(`/tools/video-caption-burner/?${params.toString()}`);
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-position')).toHaveValue('top');
  await expect(page.locator('#in-font_color')).toHaveValue('#ff0');
  await expect(page.locator('#in-background')).not.toBeChecked();
  await page.fill('#in-subtitles', 'WEBVTT\n\n00:00.000 --> 00:00:03.000\nTOP');
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.0);
  expect(f.yellowTop).toBeGreaterThan(10);
  expect(f.yellowBottom).toBe(0);
  expect(f.white).toBeLessThan(f.yellow);
});

test('video-caption-burner draws a named-color background bar at center', async ({ page }) => {
  await page.goto('/tools/video-caption-burner/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-subtitles', '1\n00:00:00,000 --> 00:00:03,000\nCENTER');
  await page.selectOption('#in-position', 'center');
  await page.fill('#in-font_color', 'white');
  await expect(page.locator('#in-background')).toBeChecked();
  await page.fill('#in-background_color', 'navy');
  await page.fill('#in-background_opacity', '1'); // exact cap boundary
  await page.setInputFiles('#in-file', BLACK);

  const src = await outputSrc(page);
  const f = await frameStats(page, src, 1.0);
  expect(f.navyTop).toBeGreaterThan(0);
  expect(f.navyBottom).toBeGreaterThan(0);
  expect(f.white).toBeGreaterThan(10);
});

test('video-caption-burner accepts WebM input and exposes page metadata', async ({ page }) => {
  await page.goto('/tools/video-caption-burner/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-subtitles', '1\n00:00:00,000 --> 00:00:01,000\nWEBM');
  await page.setInputFiles('#in-file', WEBM);

  const src = await outputSrc(page);
  const dims = await page.evaluate(async (dataUrl) => {
    const v = document.createElement('video');
    v.src = dataUrl;
    await new Promise<void>((res, rej) => { v.onloadeddata = () => res(); v.onerror = () => rej(new Error('decode')); });
    return { w: v.videoWidth, h: v.videoHeight, d: v.duration };
  }, src);
  expect(dims.w).toBeGreaterThan(0);
  expect(dims.h).toBeGreaterThan(0);

  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain("gizza tool video-caption-burner");
  await expect(page.locator('#in-position option[value="bottom"]')).toHaveText('Bottom (default)');
  await expect(page.locator('.tool-example-chip')).toHaveCount(4);
});
