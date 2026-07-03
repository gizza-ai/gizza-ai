import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-pitch-shift/ page pitch-shifts an uploaded audio
// file in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network).
// Output is audio, so the media element is an <audio> with a data:audio/ src.
// Beyond the mime prefix, each test decodes the result with WebAudio and
// asserts the DOMINANT FREQUENCY actually moved by the requested factor while
// the duration stayed put (media-correctness rule): the fixture is a pure
// 440 Hz tone, so +12 semitones must read ~880 Hz and -12 must read ~220 Hz,
// both still ~3 s long. Frequency is estimated by zero-crossing count over a
// 1 s mid-clip window — exact for a mono sine, immune to encoder gain.

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

test('audio-pitch-shift page shifts a 440 Hz tone up an octave to ~880 Hz, same length', async ({ page }) => {
  await page.goto('/tools/audio-pitch-shift/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-semitones', '12');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { freq, duration } = await freqAndDuration(page, src!);
  expect(freq).toBeGreaterThan(850); // pre-measured 880.0 Hz with local ffmpeg
  expect(freq).toBeLessThan(910);
  expect(duration).toBeGreaterThan(2.7); // tempo preserved: still ~3 s, not 1.5 s
  expect(duration).toBeLessThan(3.3);
});

test('audio-pitch-shift deep link prefills and shifts down an octave to ~220 Hz wav', async ({ page }) => {
  await page.goto('/tools/audio-pitch-shift/?semitones=-12&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-semitones')).toHaveValue('-12', { timeout: 15_000 });
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-audio', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  const { freq, duration } = await freqAndDuration(page, src!);
  expect(freq).toBeGreaterThan(205); // pre-measured 220.0 Hz with local ffmpeg
  expect(freq).toBeLessThan(235);
  expect(duration).toBeGreaterThan(2.7); // tempo preserved: still ~3 s, not 6 s
  expect(duration).toBeLessThan(3.3);
});

test('audio-pitch-shift bare upload gets the guiding no-op error', async ({ page }) => {
  await page.goto('/tools/audio-pitch-shift/');
  await page.waitForSelector('#in-audio');
  await page.setInputFiles('#in-audio', FIXTURE); // semitones left empty → 0
  const out = page.locator('#tool-output');
  await expect(out).toContainText('leaves the pitch unchanged', { timeout: 90_000 });
  await expect(out).toContainText('octave up');
});
