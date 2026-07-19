import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-time-stretch/ page speeds an uploaded audio file
// up or slows it down in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network). Output is audio, so the media element is an <audio> with a
// data:audio/ src. Beyond the mime prefix, each test decodes the result with
// WebAudio and asserts the DURATION actually moved by the requested factor
// while the DOMINANT FREQUENCY stayed put (media-correctness rule — the inverse
// of audio-pitch-shift): the fixture is a pure 440 Hz tone, so 2x must halve the
// duration to ~1.5 s and 0.5x must double it to ~6 s, both still reading ~440 Hz.
// Frequency is estimated by zero-crossing count over a 1 s mid-clip window —
// exact for a mono sine, immune to encoder gain.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 440 Hz mono sine, 3.03 s

async function freqAndDuration(
  page: Page,
  src: string,
): Promise<{ freq: number; duration: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    const sr = decoded.sampleRate;
    // 1 s window starting a third of the way in — clear of edge fades/padding.
    const start = Math.floor(data.length / 3);
    const end = Math.min(start + sr, data.length);
    let zc = 0;
    for (let i = start + 1; i < end; i++) {
      if (data[i - 1] < 0 !== data[i] < 0) zc++;
    }
    const seconds = (end - start) / sr;
    return { freq: zc / 2 / seconds, duration: decoded.duration };
  }, src);
}

test('audio-time-stretch page doubles the speed of a 3 s tone to ~1.5 s, pitch unchanged', async ({ page }) => {
  await page.goto('/tools/audio-time-stretch/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-factor', '2');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { freq, duration } = await freqAndDuration(page, src!);
  expect(duration).toBeGreaterThan(1.3); // pre-measured ~1.51 s with local ffmpeg
  expect(duration).toBeLessThan(1.7);
  expect(freq).toBeGreaterThan(410); // pitch preserved: still ~440 Hz, not 880
  expect(freq).toBeLessThan(470);
});

test('audio-time-stretch deep link prefills and halves the speed to ~6 s wav', async ({ page }) => {
  await page.goto('/tools/audio-time-stretch/?factor=0.5&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-factor')).toHaveValue('0.5', { timeout: 15_000 });
  await expect(page.locator('#in-factor-slider')).toHaveValue('0.5'); // slider mirrors the prefill
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { freq, duration } = await freqAndDuration(page, src!);
  expect(duration).toBeGreaterThan(5.5); // pre-measured ~6.06 s with local ffmpeg
  expect(duration).toBeLessThan(6.6);
  expect(freq).toBeGreaterThan(410); // pitch preserved: still ~440 Hz, not 220
  expect(freq).toBeLessThan(470);
});

test('audio-time-stretch bare upload gets the guiding positive-number error', async ({ page }) => {
  await page.goto('/tools/audio-time-stretch/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE); // factor left empty → 0
  const out = page.locator('#tool-output');
  await expect(out).toContainText('must be a positive number', { timeout: 90_000 });
});

test('audio-time-stretch factor of 1 gets the no-op error', async ({ page }) => {
  await page.goto('/tools/audio-time-stretch/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-factor', '1');
  await page.setInputFiles('#in-audio', FIXTURE);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('nothing to do', { timeout: 90_000 });
  await expect(out).toContainText('speed up');
});

test('audio-time-stretch preset chip re-runs after an error (2× → ~1.5 s)', async ({ page }) => {
  await page.goto('/tools/audio-time-stretch/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE); // empty factor → guiding error
  const out = page.locator('#tool-output');
  await expect(out).toContainText('must be a positive number', { timeout: 90_000 });
  await page.getByRole('button', { name: '2× (double speed)' }).click();
  await expect(page.locator('#in-factor')).toHaveValue('2');
  await expect(page.locator('#in-factor-slider')).toHaveValue('2'); // chip re-syncs the slider
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const { freq, duration } = await freqAndDuration(page, (await media.getAttribute('src'))!);
  expect(duration).toBeGreaterThan(1.3);
  expect(duration).toBeLessThan(1.7);
  expect(freq).toBeGreaterThan(410);
  expect(freq).toBeLessThan(470);
});
