import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-fade/ page fades an uploaded file in-browser via
// ffmpeg (@ffmpeg/core from jsDelivr — needs network). The fixture is a
// constant 3 s tone, so a fade is provable from the output's RMS ENVELOPE:
// with 1 s fades the first/last 0.2 s windows drop to ~0.115× the untouched
// middle (pre-measured -40.3 dB vs -21.5 dB with local ffmpeg), while the
// middle window must stay ≈ the input's (proving the fade didn't just turn
// everything down). Window ratios are immune to fixture amplitude.

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-3s.mp3'); // 3.03 s tone

/// RMS of one channel over [start, end] seconds; `source` is either a base64
/// string of the file bytes or a data: URL of the tool output.
async function windowRms(
  page: Page,
  source: { b64?: string; dataUrl?: string },
  start: number,
  end: number
): Promise<number> {
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
      return Math.sqrt(sum / Math.max(1, e - s));
    },
    { ...source, start, end }
  );
}

test('audio-fade page ramps both ends with 1 s fades, middle untouched', async ({ page }) => {
  await page.goto('/tools/audio-fade/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-fade_in', '1');
  await page.fill('#in-fade_out', '1');
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const inputMid = await windowRms(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const head = await windowRms(page, { dataUrl: src! }, 0, 0.2);
  const mid = await windowRms(page, { dataUrl: src! }, 1.4, 1.6);
  const tail = await windowRms(page, { dataUrl: src! }, 2.8, 3.0);
  expect(head / inputMid).toBeLessThan(0.3); // pre-measured ×0.115
  expect(tail / inputMid).toBeLessThan(0.3);
  expect(mid / inputMid).toBeGreaterThan(0.85); // fade ≠ volume cut
  expect(mid / inputMid).toBeLessThan(1.15);
});

test('audio-fade deep link fades only the start (fade_out=0)', async ({ page }) => {
  await page.goto('/tools/audio-fade/?fade_in=2&fade_out=0&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-fade_in')).toHaveValue('2', { timeout: 15_000 });
  await expect(page.locator('#in-fade_out')).toHaveValue('0');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const inputHead = await windowRms(page, { b64 }, 0, 0.3);
  const inputTail = await windowRms(page, { b64 }, 2.7, 3.0);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const head = await windowRms(page, { dataUrl: src! }, 0, 0.3);
  const tail = await windowRms(page, { dataUrl: src! }, 2.7, 3.0);
  expect(head / inputHead).toBeLessThan(0.3); // 2 s ramp ⇒ ×0.087 over 0-0.3 s
  expect(tail / inputTail).toBeGreaterThan(0.85); // end untouched, wav exact
  expect(tail / inputTail).toBeLessThan(1.15);
});
