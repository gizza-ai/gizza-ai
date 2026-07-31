import { test, expect } from './fixtures';
import type { Page } from '@playwright/test';
import path from 'node:path';

const FIXTURE = path.resolve(__dirname, 'fixtures/tone-quiet-3s.mp3');

async function decodeDuration(page: Page, src: string): Promise<{ duration: number; bytes: number }> {
  return page.evaluate(async (dataUrl: string) => {
    const [, base64 = ''] = dataUrl.split(',', 2);
    const bin = atob(base64);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    const ctx = new AudioContext();
    const decoded = await ctx.decodeAudioData(bytes.buffer.slice(0));
    await ctx.close();
    return { duration: decoded.duration, bytes: bytes.byteLength };
  }, src);
}

async function expectPlayableOutput(page: Page, mimePrefix: RegExp) {
  const media = page.locator('#tool-output-media');
  await expect(media).toBeVisible({ timeout: 90_000 });
  let src = await media.getAttribute('src');
  if (!src || src.endsWith('base64,')) {
    src = await page.getByRole('link', { name: 'Download' }).getAttribute('href');
  }
  expect(src).toMatch(mimePrefix);
  const { duration, bytes } = await decodeDuration(page, src!);
  expect(bytes).toBeGreaterThan(2_000);
  expect(duration).toBeGreaterThan(2.7);
  expect(duration).toBeLessThan(3.4);
}

test('audio-noise-gate page applies the default gate and preserves duration', async ({ page }) => {
  await page.goto('/tools/audio-noise-gate/');
  await page.waitForSelector('#in-file');
  await page.setInputFiles('#in-file', FIXTURE);
  await expectPlayableOutput(page, /^data:audio\/mpeg/);
});

test('audio-noise-gate deep link honors hard-gate controls and wav output', async ({ page }) => {
  await page.goto('/tools/audio-noise-gate/?threshold=-30&reduction=80&ratio=10&attack=5&release=150&detection=peak&format=wav');
  await page.waitForSelector('#in-file');
  await expect(page.locator('#in-threshold')).toHaveValue('-30', { timeout: 15_000 });
  await expect(page.locator('#in-reduction')).toHaveValue('80');
  await expect(page.locator('#in-ratio')).toHaveValue('10');
  await expect(page.locator('#in-attack')).toHaveValue('5');
  await expect(page.locator('#in-release')).toHaveValue('150');
  await expect(page.locator('#in-detection')).toHaveValue('peak');
  await expect(page.locator('#in-format')).toHaveValue('wav');
  await page.setInputFiles('#in-file', FIXTURE);
  await expectPlayableOutput(page, /^data:audio\/wav/);
});
