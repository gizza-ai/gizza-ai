import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-loop/ page repeats an uploaded clip in-browser
// via ffmpeg (@ffmpeg/core from jsDelivr — needs network). The fixture is a
// constant 3.03 s tone, so looping is provable two ways: the output duration
// must match the requested mode (an exact -t cut, or clip-length × plays),
// and a window deep inside a LATER repetition must carry the same RMS as the
// input's middle — silence-padding or a single play would fail that.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 3.03 s tone

/// { duration, rms of [start, end] } of one channel; `source` is a base64
/// string of the file bytes or a data: URL of the tool output.
async function decodeWindow(
  page: Page,
  source: { b64?: string; dataUrl?: string },
  start: number,
  end: number
): Promise<{ duration: number; rms: number }> {
  return page.evaluate(
    async ({ b64, dataUrl, start, end }) => {
      let buf: ArrayBuffer;
      if (dataUrl) {
        buf = await (await fetch(dataUrl)).arrayBuffer();
      } else {
        const bin = atob(b64!);
        const arr = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
        buf = arr.buffer;
      }
      const ctx = new AudioContext();
      const decoded = await ctx.decodeAudioData(buf);
      await ctx.close();
      const data = decoded.getChannelData(0);
      const s = Math.floor(start * decoded.sampleRate);
      const e = Math.min(Math.floor(end * decoded.sampleRate), data.length);
      let sum = 0;
      for (let i = s; i < e; i++) sum += data[i] * data[i];
      return { duration: decoded.duration, rms: Math.sqrt(sum / Math.max(1, e - s)) };
    },
    { ...source, start, end }
  );
}

test('audio-loop page fills 10 s from a 3 s tone, later plays carry the sound', async ({ page }) => {
  await page.goto('/tools/audio-loop/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-duration', '10');
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  // Window 4-6 s sits inside the second repetition of the 3.03 s clip.
  const out = await decodeWindow(page, { dataUrl: src! }, 4, 6);
  expect(out.duration).toBeGreaterThan(9.8);
  expect(out.duration).toBeLessThan(10.3);
  expect(out.rms / input.rms).toBeGreaterThan(0.85); // real repeats, not padding
  expect(out.rms / input.rms).toBeLessThan(1.15);
});

test('audio-loop deep link plays the clip exactly 3× (duration=0&count=3)', async ({ page }) => {
  await page.goto('/tools/audio-loop/?duration=0&count=3&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-duration')).toHaveValue('0', { timeout: 15_000 });
  await expect(page.locator('#in-count')).toHaveValue('3');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  // 3 full plays of a 3.03 s clip ⇒ ~9.1 s, never cut mid-play.
  const out = await decodeWindow(page, { dataUrl: src! }, 7.5, 8.5);
  expect(out.duration).toBeGreaterThan(8.8);
  expect(out.duration).toBeLessThan(9.5);
  expect(out.rms / input.rms).toBeGreaterThan(0.85); // third play carries the tone
  expect(out.rms / input.rms).toBeLessThan(1.15);
});
