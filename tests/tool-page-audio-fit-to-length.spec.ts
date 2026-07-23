import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

// The generated /tools/audio-fit-to-length/ page pads-with-silence or trims an
// uploaded clip in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs
// network) to hit an EXACT total length. The fixture is a constant 3.03 s tone,
// so fitting is provable from both the decoded duration and the RMS ENVELOPE:
// padding to 6 s adds ~3 s of silence, so a window inside the silence must be
// ~0 while a window over the tone must keep the input's middle RMS. pad=end
// puts the silence AFTER the clip (later window silent); pad=start puts it
// BEFORE (leading window silent, tone shifted later). Window ratios are immune
// to fixture amplitude; small codec noise is tolerated.

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

test('audio-fit-to-length pads a 3 s tone to exactly 6 s with silence at the end', async ({ page }) => {
  await page.goto('/tools/audio-fit-to-length/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-duration', '6'); // pad defaults to end
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const out = await decodeWindow(page, { dataUrl: src! }, 1.0, 2.0);
  // 3.03 s tone + ~2.97 s trailing silence = ~6 s total.
  expect(out.duration).toBeGreaterThan(5.9);
  expect(out.duration).toBeLessThan(6.2);
  // Window over the original tone keeps the input's amplitude...
  expect(out.rms / input.rms).toBeGreaterThan(0.85);
  expect(out.rms / input.rms).toBeLessThan(1.15);
  // ...while a window 4-5.5 s in — past the 3.03 s clip — is padded silence.
  const tail = await decodeWindow(page, { dataUrl: src! }, 4.0, 5.5);
  expect(tail.rms / input.rms).toBeLessThan(0.1);
});

test('audio-fit-to-length deep link pads at the start, tone appears later (pad=start, wav)', async ({ page }) => {
  await page.goto('/tools/audio-fit-to-length/?duration=6&pad=start&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-duration')).toHaveValue('6', { timeout: 15_000 });
  await expect(page.locator('#in-pad')).toHaveValue('start');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  const b64 = fs.readFileSync(FIXTURE).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const out = await decodeWindow(page, { dataUrl: src! }, 4.0, 5.5);
  // ~2.97 s leading silence + 3.03 s tone = ~6 s total (wav is sample-exact).
  expect(out.duration).toBeGreaterThan(5.9);
  expect(out.duration).toBeLessThan(6.1);
  // The leading window is prepended silence...
  const head = await decodeWindow(page, { dataUrl: src! }, 0.5, 2.0);
  expect(head.rms / input.rms).toBeLessThan(0.1);
  // ...and the tone, shifted after the silence, carries the input amplitude.
  expect(out.rms / input.rms).toBeGreaterThan(0.85);
  expect(out.rms / input.rms).toBeLessThan(1.15);
});
