import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-limiter/ page applies ffmpeg alimiter in-browser
// (@ffmpeg/core from jsDelivr — needs network). The fixture is a quiet 3s tone
// (RMS ≈ 0.0042). The default run proves a real encode with duration preserved;
// the deep-link run applies +12 dB drive to wav output and decodes the result so
// the control effect is measured, not just that an audio element appeared.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

async function decodeStats(page: Page, src: string): Promise<{ duration: number; rms: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const bytes = buf.byteLength;
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return { duration: decoded.duration, rms: Math.sqrt(sum / data.length), bytes };
  }, src);
}

test('audio-limiter page runs the default limiter and preserves the audio', async ({ page }) => {
  await page.goto('/tools/audio-limiter/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE); // controls blank → defaults, mp3
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeStats(page, src!);
  expect(bytes).toBeGreaterThan(2_000); // a real encode, not a stub
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
});

test('audio-limiter deep link prefills controls and applies +12 dB drive (wav)', async ({ page }) => {
  await page.goto('/tools/audio-limiter/?ceiling=-1&gain=12&attack=5&release=80&smooth_release=true&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-ceiling')).toHaveValue('-1', { timeout: 15_000 });
  await expect(page.locator('#in-gain')).toHaveValue('12');
  await expect(page.locator('#in-attack')).toHaveValue('5');
  await expect(page.locator('#in-release')).toHaveValue('80');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, rms } = await decodeStats(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
  expect(rms).toBeGreaterThan(0.012); // input RMS ≈ 0.0042, +12 dB ≈ ×3.98 → ≈ 0.0167
  expect(rms).toBeLessThan(0.022);
});
