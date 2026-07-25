import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-mute-section/ page silences a video's audio over
// ONE chosen [start, end] window in-browser via ffmpeg (@ffmpeg/core from
// jsDelivr — needs network). The picture is stream-copied (`-c:v copy`) and only
// the audio is re-encoded, so the output keeps the mp4 container and resolves to
// a data:video/mp4 URL. The fixture is a steady ~2s tone-over-video clip, so
// after silencing [0.5, 1.5] we can measure per-window audio RMS: the muted
// middle collapses to near-silence while the untouched edges stay audible —
// proving the range was really acted on, not just re-encoded. We also assert the
// deep-link query params (?start=&end=) pre-fill the fields.

const FIXTURE = path.resolve(__dirname, 'fixtures/tiny-128x128-audio.mp4');

async function decodeInfo(
  page: Page,
  src: string
): Promise<{ vw: number; vh: number; duration: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const bytes = buf.byteLength;
    const v = document.createElement('video');
    v.muted = true;
    v.src = dataUrl;
    await new Promise((resolve, reject) => {
      v.addEventListener('loadeddata', resolve, { once: true });
      v.addEventListener('error', () => reject(new Error('muted video failed to decode')), { once: true });
    });
    return { vw: v.videoWidth, vh: v.videoHeight, duration: v.duration, bytes };
  }, src);
}

async function windowRms(
  page: Page,
  src: string,
  start: number,
  end: number
): Promise<number> {
  return page.evaluate(
    async ({ dataUrl, start, end }) => {
      const res = await fetch(dataUrl);
      const buf = await res.arrayBuffer();
      const ctx = new AudioContext();
      const decoded = await ctx.decodeAudioData(buf);
      await ctx.close();
      const data = decoded.getChannelData(0);
      const sr = decoded.sampleRate;
      const a = Math.max(0, Math.floor(start * sr));
      const b = Math.min(data.length, Math.floor(end * sr));
      let sum = 0;
      for (let i = a; i < b; i++) sum += data[i] * data[i];
      return Math.sqrt(sum / Math.max(1, b - a));
    },
    { dataUrl: src, start, end }
  );
}

test('video-mute-section page silences only the chosen window and keeps the picture', async ({ page }) => {
  await page.goto('/tools/video-mute-section/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.setInputFiles('#in-file', FIXTURE);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  // Real re-mux with an intact, decodable picture — the video is untouched.
  const { vw, vh, duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(5_000);
  expect(vw).toBe(128);
  expect(vh).toBe(128);
  expect(duration).toBeGreaterThan(1.8); // nothing outside the window is cut
  expect(duration).toBeLessThan(2.3);

  // Per-window audio proof: edges audible, the [0.5,1.5] window silenced.
  const before = await windowRms(page, src!, 0.05, 0.4);
  const middle = await windowRms(page, src!, 0.7, 1.3);
  const after = await windowRms(page, src!, 1.6, 1.95);
  expect(before).toBeGreaterThan(0.03); // the tone really is there before...
  expect(after).toBeGreaterThan(0.03); // ...and after the window
  expect(middle).toBeLessThan(0.01); // ...and the window itself is silent
  expect(before).toBeGreaterThan(middle * 5);
  expect(after).toBeGreaterThan(middle * 5);
});

test('video-mute-section deep link pre-fills the start/end fields', async ({ page }) => {
  await page.goto('/tools/video-mute-section/?start=0.5&end=1.5');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-start')).toHaveValue('0.5', { timeout: 15_000 });
  await expect(page.locator('#in-end')).toHaveValue('1.5');

  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:video\/mp4/);

  const before = await windowRms(page, src!, 0.05, 0.4);
  const middle = await windowRms(page, src!, 0.7, 1.3);
  expect(before).toBeGreaterThan(0.03);
  expect(middle).toBeLessThan(0.01);
});
