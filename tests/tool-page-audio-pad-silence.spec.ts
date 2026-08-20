import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';
import fs from 'node:fs';

const FIXTURE_MP3 = path.resolve(__dirname, 'fixtures/tone-3s.mp3');
const FIXTURE_WAV = path.resolve(__dirname, 'fixtures/tone-3s.wav');

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

test('audio-pad-silence pads both ends and keeps the tone in the middle', async ({ page }) => {
  await page.goto('/tools/audio-pad-silence/');
  await page.waitForSelector('#in-audio');
  await page.fill('#in-start', '0.5');
  await page.fill('#in-end', '1.5');
  await page.selectOption('#in-format', 'mp3');

  const b64 = fs.readFileSync(FIXTURE_MP3).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE_MP3);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);

  const out = await decodeWindow(page, { dataUrl: src! }, 0.8, 2.2);
  expect(out.duration).toBeGreaterThan(4.8);
  expect(out.duration).toBeLessThan(5.4);
  expect(out.rms / input.rms).toBeGreaterThan(0.75);
  expect(out.rms / input.rms).toBeLessThan(1.25);

  const head = await decodeWindow(page, { dataUrl: src! }, 0.05, 0.35);
  expect(head.rms / input.rms).toBeLessThan(0.1);
  const tail = await decodeWindow(page, { dataUrl: src! }, 3.8, 4.7);
  expect(tail.rms / input.rms).toBeLessThan(0.1);
});

test('audio-pad-silence deep link prefills tail-only WAV settings', async ({ page }) => {
  await page.goto('/tools/audio-pad-silence/?start=0&end=2&format=wav');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-start')).toHaveValue('0', { timeout: 15_000 });
  await expect(page.locator('#in-end')).toHaveValue('2');
  await expect(page.locator('#in-format')).toHaveValue('wav');

  const b64 = fs.readFileSync(FIXTURE_MP3).toString('base64');
  const input = await decodeWindow(page, { b64 }, 1.4, 1.6);
  await page.setInputFiles('#in-audio', FIXTURE_MP3);

  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/wav/);

  const out = await decodeWindow(page, { dataUrl: src! }, 1.0, 2.0);
  expect(out.duration).toBeGreaterThan(4.9);
  expect(out.duration).toBeLessThan(5.2);
  expect(out.rms / input.rms).toBeGreaterThan(0.75);
  expect(out.rms / input.rms).toBeLessThan(1.25);
  const tail = await decodeWindow(page, { dataUrl: src! }, 3.6, 4.7);
  expect(tail.rms / input.rms).toBeLessThan(0.1);
});

test('audio-pad-silence advertised formats and secondary wav input stay wired', async ({ page }) => {
  await page.goto('/tools/audio-pad-silence/?start=0.25&end=0&format=flac');
  await page.waitForSelector('#in-audio');
  await expect(page.locator('#in-start')).toHaveValue('0.25', { timeout: 15_000 });
  await expect(page.locator('#in-end')).toHaveValue('0');
  for (const format of ['mp3', 'wav', 'ogg', 'flac', 'm4a']) {
    await page.selectOption('#in-format', format);
    await expect(page.locator('#in-format')).toHaveValue(format);
  }
  await page.selectOption('#in-format', 'flac');
  await page.setInputFiles('#in-audio', FIXTURE_WAV);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\/(flac|x-flac)/);
  const out = await decodeWindow(page, { dataUrl: src! }, 0.35, 1.0);
  expect(out.duration).toBeGreaterThan(3.15);
  expect(out.duration).toBeLessThan(3.5);
  expect(out.rms).toBeGreaterThan(0.001);
});

test('audio-pad-silence generated CLI example is generic and parseable', async ({ page }) => {
  await page.goto('/tools/audio-pad-silence/');
  const cli = (await page.locator('.tool-cli-code').first().textContent())!.trim();
  expect(cli).toContain('gizza tool audio-pad-silence');
  expect(cli).toContain('url=https://example.com/input');
  expect(cli).toContain('start=2');
  expect(cli).toContain('end=0');
  expect(cli).toContain('format=mp3');
  expect(cli).not.toContain('TODO');
  expect(cli).not.toContain('gizza.ai');
});
