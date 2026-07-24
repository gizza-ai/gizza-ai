import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-bleep-censor/ page censors listed time regions of
// an uploaded file in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network): bleep mixes a tone over the region, mute silences it, duck lowers
// it. The fixture is a steady 3s 440Hz tone, so after censoring a 1s region the
// output must still decode to a full ~3s clip (nothing outside the region is
// touched). For mute we go further and measure per-window RMS: the censored
// [1,2] window collapses to near-silence while the untouched edges stay loud,
// which proves the region was actually acted on rather than just re-encoded.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3');

async function decodeInfo(
  page: Page,
  src: string
): Promise<{ duration: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const bytes = buf.byteLength;
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    return { duration: decoded.duration, bytes };
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

test('audio-bleep-censor page bleeps a region and keeps the full clip', async ({ page }) => {
  await page.goto('/tools/audio-bleep-censor/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-regions', '1.0-2.0'); // default mode bleep, mp3 out
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(5_000); // a real encode, not a stub
  expect(duration).toBeGreaterThan(2.9); // nothing outside the region is cut
  expect(duration).toBeLessThan(3.2);
});

test('audio-bleep-censor deep link prefills mute + wav and silences only the region', async ({ page }) => {
  await page.goto('/tools/audio-bleep-censor/?regions=1.0-2.0&mode=mute&tone_hz=1500&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-regions')).toHaveValue('1.0-2.0', { timeout: 15_000 });
  await expect(page.locator('#in-mode')).toHaveValue('mute');
  await expect(page.locator('#in-tone_hz')).toHaveValue('1500');
  await expect(page.locator('#in-format')).toHaveValue('wav');

  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);

  const { duration, bytes } = await decodeInfo(page, src!);
  expect(bytes).toBeGreaterThan(5_000);
  expect(duration).toBeGreaterThan(2.9); // muting keeps the clip length
  expect(duration).toBeLessThan(3.2);

  const before = await windowRms(page, src!, 0.1, 0.9); // untouched tone
  const middle = await windowRms(page, src!, 1.1, 1.9); // muted region
  const after = await windowRms(page, src!, 2.1, 2.9); // untouched tone
  expect(before).toBeGreaterThan(0.05); // the tone really is there
  expect(after).toBeGreaterThan(0.05);
  expect(middle).toBeLessThan(0.02); // ...and the region was silenced
  expect(before).toBeGreaterThan(middle * 5);
  expect(after).toBeGreaterThan(middle * 5);
});
