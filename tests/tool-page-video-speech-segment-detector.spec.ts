import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/video-speech-segment-detector/ page runs a DETECT-only
// ffmpeg pass (silencedetect, @ffmpeg/core from jsDelivr — needs network) and
// renders the speech / non-speech timestamps as TEXT via page/custom.js (the
// video-autocrop-bars takeover shape). The video is never re-encoded.
//
// Fixtures (generated with ffmpeg, bounds PRE-MEASURED on these exact files
// with native silencedetect at the defaults -30 dB / 0.5 s):
//   speech-gaps-6s.mp4   — 6s clip, 440 Hz tone 0–2s, silence 2–4s, tone 4–6s
//       (h264 + AAC) → silence_start ≈ 2.021, silence_end ≈ 4.017
//   speech-gaps-6s.webm  — same timeline, VP8 + Vorbis (secondary container)
//       → silence_start ≈ 2.024, silence_end ≈ 4.020
//   tiny-128x128.mp4     — video-only, NO audio stream (error path).
// The wasm decoder may land within a few hundredths of native; assertions use
// ±0.25 s tolerance around the pre-measured boundaries.

const MP4 = path.resolve(__dirname, 'fixtures/speech-gaps-6s.mp4');
const WEBM = path.resolve(__dirname, 'fixtures/speech-gaps-6s.webm');
const NO_AUDIO = path.resolve(__dirname, 'fixtures/tiny-128x128.mp4');

const output = (page: Page) => page.locator('#tool-output');

async function outputText(page: Page, matcher: RegExp): Promise<string> {
  await expect(output(page)).toContainText(matcher, { timeout: 120_000 });
  return (await output(page).textContent()) ?? '';
}

test('default report labels tone/silence/tone with real boundaries', async ({ page }) => {
  await page.goto('/tools/video-speech-segment-detector/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', MP4);

  const text = await outputText(page, /speech: 2 segments/);
  // Summary — both classes, real seconds. ~4.0s of 6.0s speech.
  expect(text).toMatch(/^speech: 2 segments, [34]\.\d{3}s of 6\.0\d{2}s \(6[0-9]\.\d%\)/);
  expect(text).toContain('non-speech: 1 segment,');
  // The listed timeline is exactly speech → non-speech → speech.
  const rows = [...text.matchAll(/#\d+ {2}(\d+:\d{2}\.\d{3}) → (\d+:\d{2}\.\d{3}) {2}(speech|non-speech)/g)];
  expect(rows.map((r) => r[3])).toEqual(['speech', 'non-speech', 'speech']);
  // Boundary times match the pre-measured silencedetect values (±0.25 s).
  const secs = (clock: string) => {
    const [m, s] = clock.split(':');
    return Number(m) * 60 + Number(s);
  };
  expect(Math.abs(secs(rows[0][2]) - 2.02)).toBeLessThan(0.25); // speech ends
  expect(Math.abs(secs(rows[1][2]) - 4.02)).toBeLessThan(0.25); // silence ends
  expect(Math.abs(secs(rows[2][2]) - 6.0)).toBeLessThan(0.1); // clip end
  // Text result: download link ready, media element never used.
  await expect(page.locator('#tool-output-download')).toBeVisible();
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'speech-segments.txt');
  await expect(page.locator('#tool-output-media')).toBeHidden();

  // Same page, output=csv (enum run): header + 3 labeled rows, .csv download.
  await page.selectOption('#in-output', 'csv');
  const csv = await outputText(page, /start,end,duration,label/);
  const csvRows = csv.trim().split('\n').slice(1);
  expect(csvRows).toHaveLength(3);
  expect(csvRows[0]).toMatch(/^0\.000,\d\.\d{3},\d\.\d{3},speech$/);
  expect(csvRows[1]).toMatch(/,non-speech$/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'speech-segments.csv');

  // Same page, segments=speech (enum run): only the 2 speech rows remain.
  await page.selectOption('#in-segments', 'speech');
  await expect
    .poll(
      async () => {
        const t = (await output(page).textContent()) ?? '';
        return t.startsWith('start,end') ? t.trim().split('\n').length : 0;
      },
      { timeout: 120_000 }
    )
    .toBe(3); // header + 2 speech rows
  expect((await output(page).textContent()) ?? '').not.toContain('non-speech');

  // Same page, threshold at the slider top (-10 dB): the -18 dB tone now reads
  // as silence → zero speech segments + the report's explicit hint.
  await page.selectOption('#in-output', 'report');
  await page.locator('#in-threshold_db').fill('-10');
  await page.locator('#in-threshold_db').dispatchEvent('change');
  const none = await outputText(page, /No speech detected/);
  expect(none).toContain('speech: 0 segments, 0.000s of 6.0');
});

test('srt + audacity outputs with padding (subtitle prep preset values)', async ({ page }) => {
  await page.goto('/tools/video-speech-segment-detector/');
  await page.waitForSelector('#in-file');
  await page.selectOption('#in-output', 'srt');
  await page.selectOption('#in-segments', 'speech');
  await page.fill('#in-pad', '0.2');
  await page.setInputFiles('#in-file', MP4);

  const srt = await outputText(page, /--> /);
  // Two speech entries, padded by 0.2s: 0.0–~2.22 and ~3.82–6.0.
  expect(srt).toMatch(/^1\n00:00:00,000 --> 00:00:02,[1-4]\d{2}\nspeech\n/);
  expect(srt).toMatch(/\n2\n00:00:03,[6-9]\d{2} --> 00:00:0[56],\d{3}\nspeech\n/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'speech-segments.srt');

  // Same page, audacity label track (enum run): tab-separated, 6-decimal rows.
  await page.selectOption('#in-output', 'audacity');
  const labels = await outputText(page, /\t/);
  expect(labels).toMatch(/^0\.000000\t2\.[1-4]\d{5}\tspeech\n/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'speech-segments.txt');
});

test('webm input, voice-band OFF (non-default checkbox), non-speech csv', async ({ page }) => {
  await page.goto('/tools/video-speech-segment-detector/');
  await page.waitForSelector('#in-file');
  await page.uncheck('#in-voice_band'); // default-true checkbox → off path
  await page.selectOption('#in-segments', 'non-speech');
  await page.selectOption('#in-output', 'csv');
  await page.setInputFiles('#in-file', WEBM);

  const csv = await outputText(page, /start,end,duration,label/);
  const rows = csv.trim().split('\n').slice(1);
  expect(rows).toHaveLength(1); // exactly the one silent gap
  const [start, end, , label] = rows[0].split(',');
  expect(label).toBe('non-speech');
  expect(Math.abs(Number(start) - 2.02)).toBeLessThan(0.25);
  expect(Math.abs(Number(end) - 4.02)).toBeLessThan(0.25);
});

test('deep link prefills detection params and the run reflects them', async ({ page }) => {
  await page.goto('/tools/video-speech-segment-detector/?output=csv&segments=speech&min_silence=1.5');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-output')).toHaveValue('csv', { timeout: 15_000 });
  await expect(page.locator('#in-segments')).toHaveValue('speech');
  await expect(page.locator('#in-min_silence')).toHaveValue('1.5');
  await page.setInputFiles('#in-file', MP4);

  const csv = await outputText(page, /start,end,duration,label/);
  const rows = csv.trim().split('\n').slice(1);
  expect(rows).toHaveLength(2); // speech-only rows, the 2s gap still splits them
  for (const row of rows) expect(row).toMatch(/,speech$/);
});

test('a video without an audio track fails with a clear message', async ({ page }) => {
  await page.goto('/tools/video-speech-segment-detector/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', NO_AUDIO);
  await expect(output(page)).toContainText('no audio track', { timeout: 120_000 });
  await expect(output(page)).toHaveClass(/error/);
});
