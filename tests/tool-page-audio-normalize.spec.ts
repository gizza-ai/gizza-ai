import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-normalize/ page levels an uploaded audio file
// in-browser via ffmpeg loudnorm (@ffmpeg/core from jsDelivr — needs network).
// The fixture is a quiet 3s 440 Hz tone (amplitude 0.05 ≈ -30 LUFS, RMS
// ≈ 0.035). A dense sine leveled to a target T LUFS has a predictable RMS
// (~10^((T+0.7)/20)), so decoding the output and measuring RMS proves the
// loudness actually moved to the requested target — not just that "some audio"
// came out. -14 → RMS ≈ 0.21; -23 → RMS ≈ 0.077.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

async function decodeStats(page: Page, src: string): Promise<{ duration: number; rms: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    let sum = 0;
    for (let i = 0; i < data.length; i++) sum += data[i] * data[i];
    return { duration: decoded.duration, rms: Math.sqrt(sum / data.length) };
  }, src);
}

test('audio-normalize page lifts a quiet tone to the -14 LUFS default', async ({ page }) => {
  await page.goto('/tools/audio-normalize/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, rms } = await decodeStats(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
  expect(rms).toBeGreaterThan(0.15); // input RMS ≈ 0.035 → -14 LUFS ≈ 0.21
  expect(rms).toBeLessThan(0.3);
});

test('audio-normalize deep link targets -23 LUFS broadcast as flac', async ({ page }) => {
  await page.goto('/tools/audio-normalize/?lufs=-23&format=flac');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-lufs')).toHaveValue('-23', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('flac');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, rms } = await decodeStats(page, src!);
  expect(duration).toBeGreaterThan(2.9);
  expect(duration).toBeLessThan(3.3);
  expect(rms).toBeGreaterThan(0.05); // -23 LUFS sine ≈ 0.077
  expect(rms).toBeLessThan(0.11); // clearly quieter than the -14 case
});
