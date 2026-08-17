import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-8000hz-3s.mp3');

async function decodeStats(page: Page, src: string): Promise<{ duration: number; bytes: number }> {
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

async function run(page: Page) {
  await page.setInputFiles('#in-file', FIXTURE);
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  const src = await media.getAttribute('src');
  expect(src).toMatch(/^data:audio\//);
  return { src: src!, stats: await decodeStats(page, src!) };
}

test('bitcrush page renders a crushed wav output', async ({ page }) => {
  await page.goto('/tools/bitcrush/');
  await page.waitForSelector('#in-file');
  await page.fill('#in-bits', '4');
  await page.fill('#in-sample_rate_hz', '4000');
  await page.fill('#in-mix', '1');
  await page.fill('#in-drive', '2');
  await page.fill('#in-output_gain', '0.6');
  await page.fill('#in-anti_alias', '0');
  await page.selectOption('#in-mode', 'log');
  await page.selectOption('#in-format', 'wav');
  const out = await run(page);
  expect(out.src).toMatch(/^data:audio\/(wav|x-wav)/);
  await expect(page.locator('#tool-output-download')).toHaveAttribute('download', 'out.wav');
  expect(out.stats.bytes).toBeGreaterThan(1_000);
  expect(out.stats.duration).toBeGreaterThan(2.8);
  expect(out.stats.duration).toBeLessThan(3.4);
});

test('bitcrush deep link prefills controls and exports flac', async ({ page }) => {
  await page.goto('/tools/bitcrush/?bits=12&sample_rate_hz=16000&mix=0.45&drive=1&output_gain=1&anti_alias=0.8&mode=lin&format=flac');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-bits')).toHaveValue('12', { timeout: 15_000 });
  await expect(page.locator('#in-sample_rate_hz')).toHaveValue('16000');
  await expect(page.locator('#in-mix')).toHaveValue('0.45');
  await expect(page.locator('#in-drive')).toHaveValue('1');
  await expect(page.locator('#in-output_gain')).toHaveValue('1');
  await expect(page.locator('#in-anti_alias')).toHaveValue('0.8');
  await expect(page.locator('#in-mode')).toHaveValue('lin');
  await expect(page.locator('#in-format')).toHaveValue('flac');
  const out = await run(page);
  expect(out.src).toMatch(/^data:audio\/flac/);
  expect(out.stats.bytes).toBeGreaterThan(1_000);
});
