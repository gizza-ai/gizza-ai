import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

// The generated /tools/audio-ringtone/ page cuts an uploaded song into a phone
// ringtone in-browser via ffmpeg (@ffmpeg/core from jsDelivr — needs network).
// Output is audio (m4r = AAC in the ipod/MP4 container, or mp3). Each test
// decodes the result with WebAudio and asserts duration + level, so the trim,
// the -14 LUFS loudness boost, and the edge fades are proven end-to-end — not
// just "some audio came out".
//
// Fixture: tone-quiet-3s.mp3 is a 3.03 s 440 Hz sine, measured RMS ≈ 0.004
// (lavfi sine is ~1/8 amplitude — see create-next-tool references/
// page-patterns.md). Normalized to -14 LUFS a dense sine lands at RMS ≈ 0.21
// (see tool-page-audio-normalize.spec.ts); the default 0.5 s fades shave the
// whole-clip RMS to ≈ 0.185. headRms (0.1–0.3 s) vs midRms (1–2 s) separates
// faded from unfaded starts: a linear 0.5 s fade-in puts head/mid ≈ 0.42,
// no fade ≈ 1.

const QUIET = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');
const WAV = path.resolve(__dirname, 'fixtures/tone-3s.wav');

type Stats = { duration: number; rms: number; headRms: number; midRms: number };

async function decodeStats(page: Page, src: string): Promise<Stats> {
  return page.evaluate(async (dataUrl: string) => {
    const res = await fetch(dataUrl);
    const buf = await res.arrayBuffer();
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(buf);
    await ctx.close();
    const data = decoded.getChannelData(0);
    const sr = decoded.sampleRate;
    const rmsOf = (from: number, to: number) => {
      const a = Math.min(data.length, Math.max(0, Math.round(from * sr)));
      const b = Math.min(data.length, Math.max(a + 1, Math.round(to * sr)));
      let sum = 0;
      for (let i = a; i < b; i++) sum += data[i] * data[i];
      return Math.sqrt(sum / (b - a));
    };
    return {
      duration: decoded.duration,
      rms: rmsOf(0, decoded.duration),
      headRms: rmsOf(0.1, 0.3),
      midRms: rmsOf(1.0, 2.0),
    };
  }, src);
}

test('audio-ringtone defaults: m4r, -14 LUFS boost, 0.5 s fades', async ({ page }) => {
  await page.goto('/tools/audio-ringtone/');
  await page.waitForSelector('#in-audio');
  // Defaults: start 0, end empty (= start + 30, clamped to the 3 s file),
  // fades 0.5, normalize checked, format m4r.
  await page.setInputFiles('#in-audio', QUIET);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  // audio/mp4, NOT application/octet-stream — the .m4r → mime mapping.
  expect(src).toMatch(/^data:audio\/mp4/);
  const dl = page.locator('#tool-output-download');
  await expect(dl).toHaveAttribute('download', 'out.m4r');
  const s = await decodeStats(page, src!);
  expect(s.duration).toBeGreaterThan(2.8); // 3 s source < 30 s default slice
  expect(s.duration).toBeLessThan(3.4);
  expect(s.rms).toBeGreaterThan(0.11); // quiet source (0.035) got boosted…
  expect(s.rms).toBeLessThan(0.28); // …to ≈ -14 LUFS (≈ 0.185 with fades)
  expect(s.headRms / s.midRms).toBeLessThan(0.6); // fade-in really ramps
});

test('audio-ringtone raw cut: mp3, normalize off, fades 0', async ({ page }) => {
  await page.goto('/tools/audio-ringtone/');
  await page.waitForSelector('#in-audio');
  await page.uncheck('#in-normalize'); // NON-default checkbox state
  await page.fill('#in-fade_in', '0');
  await page.fill('#in-fade_out', '0');
  await page.selectOption('#in-format', 'mp3');
  await page.setInputFiles('#in-audio', QUIET);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/mpeg/);
  const s = await decodeStats(page, src!);
  expect(s.duration).toBeGreaterThan(2.8);
  expect(s.duration).toBeLessThan(3.4);
  expect(s.rms).toBeGreaterThan(0.002); // level untouched: still the quiet
  expect(s.rms).toBeLessThan(0.02); // source RMS ≈ 0.004 — loudnorm skipped
  expect(s.headRms / s.midRms).toBeGreaterThan(0.75); // no fade-in ramp
});

test('audio-ringtone trims the [start, end] slice from a wav source', async ({ page }) => {
  // Secondary input format: wav in, m4r out.
  await page.goto('/tools/audio-ringtone/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.setInputFiles('#in-audio', WAV);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/mp4/);
  const s = await decodeStats(page, src!);
  expect(s.duration).toBeGreaterThan(0.9); // 1 s slice (AAC pads a little)
  expect(s.duration).toBeLessThan(1.3);
});

test('audio-ringtone enforces the 40 s iPhone cap (one-over errors, at-cap runs)', async ({ page }) => {
  await page.goto('/tools/audio-ringtone/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-end', '40.1'); // start 0 → 40.1 s slice: one over the cap
  await page.setInputFiles('#in-audio', QUIET);
  const out = page.locator('#tool-output');
  await expect(out).toContainText('iPhone caps ringtones at 40 seconds', { timeout: 90_000 });
  // Exactly at the cap: valid — re-runs on the field change and produces audio
  // (the 3 s file just ends early).
  await page.fill('#in-end', '40');
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/mp4/);
});

test('audio-ringtone deep link prefills and cuts a raw 1 s mp3', async ({ page }) => {
  await page.goto(
    '/tools/audio-ringtone/?start=0.5&end=1.5&fade_in=0&fade_out=0&normalize=false&format=mp3'
  );
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-start')).toHaveValue('0.5', { timeout: 15_000 });
  await expect(page.locator('#in-end')).toHaveValue('1.5');
  await expect(page.locator('#in-format')).toHaveValue('mp3');
  await expect(page.locator('#in-normalize')).not.toBeChecked();
  await page.setInputFiles('#in-audio', QUIET);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/mpeg/);
  const s = await decodeStats(page, src!);
  expect(s.duration).toBeGreaterThan(0.9);
  expect(s.duration).toBeLessThan(1.3);
  expect(s.rms).toBeLessThan(0.02); // normalize=false honored via the URL
});

test('audio-ringtone example chip prefills the raw-cut setup', async ({ page }) => {
  await page.goto('/tools/audio-ringtone/');
  await page.waitForSelector('#in-audio');
  await page.getByRole('button', { name: 'Raw cut — no boost, no fades' }).click();
  await expect(page.locator('#in-normalize')).not.toBeChecked();
  await expect(page.locator('#in-fade_in')).toHaveValue('0');
  await expect(page.locator('#in-fade_out')).toHaveValue('0');
});
