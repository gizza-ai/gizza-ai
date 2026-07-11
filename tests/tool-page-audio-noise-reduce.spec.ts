import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-noise-reduce/ page runs ffmpeg's afftdn/anlmdn
// denoiser over an uploaded clip in-browser (@ffmpeg/core from jsDelivr — needs
// network). Denoising rewrites samples but preserves duration, so the proof of a
// real output is that the returned data URL decodes to a playable ~3s audio
// buffer in the requested container.
const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.wav');

async function expectPlayableAudioDataUrl(page: Page, mimePrefix: RegExp): Promise<number> {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(mimePrefix);
  const duration = await page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return decoded.duration;
  }, src!);
  return duration;
}

test('audio-noise-reduce page denoises with the default afftdn path', async ({ page }) => {
  await page.goto('/tools/audio-noise-reduce/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  const duration = await expectPlayableAudioDataUrl(page, /^data:audio\/mpeg/);
  expect(duration).toBeGreaterThan(2.5); // ~3s tone survives, just denoised
  expect(duration).toBeLessThan(3.5);
});

test('audio-noise-reduce deep link honors strength, anlmdn, remove-hum and wav format', async ({ page }) => {
  await page.goto('/tools/audio-noise-reduce/?strength=40&method=anlmdn&remove_hum=true&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-strength')).toHaveValue('40', { timeout: 15_000 });
  await expect(page.locator('#in-method')).toHaveValue('anlmdn');
  await expect(page.locator('#in-remove_hum')).toBeChecked();
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-file', FIXTURE);
  const duration = await expectPlayableAudioDataUrl(page, /^data:audio\/wav/);
  expect(duration).toBeGreaterThan(2.5);
  expect(duration).toBeLessThan(3.5);
});
